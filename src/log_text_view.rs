use gtk::{prelude::*, TextIter, TextBuffer};
use gtk::{ScrolledWindow, Orientation, TextTag, TextTagTable};
use std::rc::Rc;
use glib::{SignalHandlerId};
use crate::util::{enable_auto_scroll, SortedListCompare, CompareResult, search, decode_data};
use crate::model::{LogTextViewMsg, LogViewData, SearchResultMatch, LogReplacer};

use sourceview::{ViewExt, BufferExt, Mark};
use regex::Regex;
use std::collections::{HashMap};
use crate::highlighters::{SEARCH_ID, Highlighter};

use log::{info, error, debug};
use crate::log_file_reader::LogFileReader;
use crate::kubernetes_log_reader::KubernetesLogReader;
use crate::model::{LogState, ActiveRule, LogReader, LogTextViewThreadMsg, RuleChanges};

pub struct LogTextView {
    container: gtk::Box,
    text_view: Rc<sourceview::View>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<Highlighter>,
    active_rule: String,
    thread_action_sender: Option<std::sync::mpsc::Sender<LogTextViewThreadMsg>>,
    result_map: HashMap<String, Vec<SearchResultMatch>>,
    current_result_selection: Option<(String, usize)>,
    result_match_cursor_pos: Option<usize>,
    marker: HashMap<u16, (i32, Mark)>,
}

const CURRENT_CURSOR_TAG: &'static str = "CURRENT_CURSOR";
const MARKER_CATEGORY_BOOKMARK: &'static str = "BOOKMARK";

impl LogTextView {
    pub fn start<T>(&mut self, data: LogViewData, sender: T, default_rules: Vec<Highlighter>)
        where T : 'static + Send + Clone + Fn(LogTextViewMsg)
    {
        let (thread_action_sender, thread_action_receiver) =
            std::sync::mpsc::channel::<LogTextViewThreadMsg>();

        self.thread_action_sender = Some(thread_action_sender);
        self.apply_rules(default_rules);

        {
            let tx = sender.clone();
            self.text_view.connect_button_press_event(move |_,_|{
                tx(LogTextViewMsg::CursorChanged);
                gtk::Inhibit(false)
            });
        }

        {
            let tx = sender.clone();
            self.text_view.connect_key_press_event(move |_, key| {
                let modifier = key.get_state();
                let key_val = key.get_keyval();
                if key_val == gdk::keys::constants::_1
                    || key_val == gdk::keys::constants::_2
                    || key_val == gdk::keys::constants::_3
                    || key_val == gdk::keys::constants::_4
                    || key_val == gdk::keys::constants::_5
                    || key_val == gdk::keys::constants::_6
                    || key_val == gdk::keys::constants::_7
                    || key_val == gdk::keys::constants::_8
                    || key_val == gdk::keys::constants::_9
                {
                    if let Some(key_code) = key.get_keycode() {
                        if modifier & gdk::ModifierType::CONTROL_MASK == gdk::ModifierType::CONTROL_MASK {
                            tx(LogTextViewMsg::ToggleBookmark(key_code));
                        }

                        if modifier & gdk::ModifierType::MOD1_MASK == gdk::ModifierType::MOD1_MASK {
                            tx(LogTextViewMsg::ScrollToBookmark(key_code));
                        }
                    }
                }

                gtk::Inhibit(false)
            });
        }

        {
            let tx = sender.clone();
            self.text_view.connect_move_cursor(move |_,_,_,_| {
                tx(LogTextViewMsg::CursorChanged);
            });
        }

        let file_thread_tx = sender.clone();
        match data {
            LogViewData::File(path) => {
                match LogFileReader::new(path) {
                    Ok(reader) => {
                        register_log_data_watcher(move |msg| {
                            file_thread_tx(msg);
                        }, Box::new(reader), thread_action_receiver);
                    }
                    Err(e) => {
                        error!("Could not open file: {}", e);
                    }
                }
            }
            LogViewData::Kube(data) => {
                let reader = KubernetesLogReader::new(data);
                register_log_data_watcher(move |msg| {
                    file_thread_tx(msg);
                }, Box::new(reader), thread_action_receiver);
            }
        }
    }
    pub fn new() -> Self {
        let tag_table = TextTagTable::new();
        let current_cursor_tag = gtk::TextTagBuilder::new()
            .name(CURRENT_CURSOR_TAG)
            .background("rgba(114,159,207,1)")
            .build();

        tag_table.add(&current_cursor_tag);
        current_cursor_tag.set_priority(tag_table.get_size() - 1);

        let text_buffer = sourceview::Buffer::new(Some(&tag_table));
        let tv = sourceview::View::new_with_buffer(&text_buffer);

        let marker_attributes = sourceview::MarkAttributesBuilder::new()
            .icon_name("pan-end-symbolic")
            .build();
        tv.set_mark_attributes(MARKER_CATEGORY_BOOKMARK, &marker_attributes, 10);

        tv.set_editable(false);
        tv.set_show_line_numbers(true);
        tv.set_child_visible(true);

        let minimap = sourceview::MapBuilder::new()
            .vexpand_set(true)
            .view(&tv)
            .width_request(220)
            .buffer(&text_buffer)
            .highlight_current_line(true)
            .build();

        minimap.set_widget_name("minimap");
        let css = r##"
            #minimap {
                  font: 1px "Monospace";
                  color: rgba(1,1,1,0.5);
            }
        "##;
        let css_provider = gtk::CssProvider::new();
        if let Ok(_) = css_provider.load_from_data(css.as_bytes()) {
            let sc = minimap.get_style_context();
            sc.add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        }

        let text_view = Rc::new(tv);
        let scroll_wnd = ScrolledWindow::new(text_view.get_hadjustment().as_ref(), text_view.get_vadjustment().as_ref());
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_hexpand(true);
        scroll_wnd.add(&*text_view);

        let container = gtk::Box::new(Orientation::Horizontal, 0);
        container.add(&scroll_wnd);
        container.add(&minimap);

        let result_map: HashMap<String, Vec<SearchResultMatch>> = HashMap::new();

        Self {
            container,
            text_view,
            autoscroll_handler: None,
            rules: vec![],
            thread_action_sender: None,
            result_map,
            active_rule: SEARCH_ID.to_string(),
            current_result_selection: None ,
            result_match_cursor_pos: None,
            marker: HashMap::new(),
        }
    }


    fn get_bounds_for_match(buffer: &TextBuffer, result: &SearchResultMatch) -> (TextIter, TextIter) {
        let line = result.line as i32;
        let iter_start = buffer.get_iter_at_line_index(line, result.start as i32);
        let iter_end = buffer.get_iter_at_line_index(line, result.end as i32);
        (iter_start, iter_end)
    }

    pub fn set_active_rule(&mut self, rule: &str) {
        self.active_rule = rule.to_string();
    }

    fn set_or_remove_selected_match(text_view: &sourceview::View, search_match: &SearchResultMatch, set: bool) {
        if let Some(buffer) = text_view.get_buffer() {
            let (mut iter_start, iter_end) = Self::get_bounds_for_match(&buffer, &search_match);
            if set {
                buffer.apply_tag_by_name(CURRENT_CURSOR_TAG, &iter_start, &iter_end);
                text_view.scroll_to_iter(&mut iter_start, 0.0, true, 0.5, 0.5);
            }else {
                buffer.remove_tag_by_name(CURRENT_CURSOR_TAG, &iter_start, &iter_end);
            }
        }
    }

    pub fn select_prev_match(&mut self, id: &String) {
        self.select_match(id, false);
    }

    pub fn select_next_match(&mut self, id: &String) {
        self.select_match(id, true);
    }

    pub fn select_match(&mut self, id: &String, forward: bool) {
        if let Some(matches) = self.result_map.get(id) {
            if let Some((current_id, current)) = self.current_result_selection.take() {
                if id != &current_id {
                    if let Some(rs) = self.result_map.get(&current_id) {
                        if let Some(c) = rs.get(current) {
                            if let Some(buffer) = self.text_view.get_buffer() {
                                let (start, _end) = Self::get_bounds_for_match(&buffer, &c);
                                self.result_match_cursor_pos = self.get_next_from_pos(&start, &id);
                            }
                            Self::set_or_remove_selected_match(&*self.text_view, &c, false);
                        }
                    }
                }

                let next = if let Some(next) = self.result_match_cursor_pos.take() {
                    next
                }else {
                    if forward {
                        if current < matches.len() -1 { current +1 } else { 0 }
                    } else {
                        if current > 0 { current -1 } else { 0 }
                    }
                };
                if let Some(prev) = matches.get(current) {
                    Self::set_or_remove_selected_match(&*self.text_view, &prev, false);
                }
                if let Some(next) = matches.get(next) {
                    Self::set_or_remove_selected_match(&*self.text_view, &next, true);
                }
                self.current_result_selection = Some((id.to_string(), next));
            }else {
                let start = if let Some(start) = self.result_match_cursor_pos.take() {
                    start
                }else {
                    if forward {
                        0
                    }else {
                        if matches.len() > 0 { matches.len() -1 } else { 0 }
                    }
                };
                if let Some(first_match) = matches.get(start) {
                    Self::set_or_remove_selected_match(&*self.text_view, &first_match, true);
                    self.current_result_selection = Some((id.to_string(), start));
                }
            }
        }
    }

    pub fn update(&mut self, msg: LogTextViewMsg) {
        match msg {
            LogTextViewMsg::Data(res) => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    let (_start, mut end) = buffer.get_bounds();
                    let offset = if res.full_search { 0 } else
                    {
                        if buffer.get_line_count() > 0 {
                            buffer.get_line_count() - 1
                        } else {
                            0
                        }
                    };
                    if res.data.len() > 0 {
                        buffer.insert(&mut end, &res.data);
                    }
                    for (tag, matches) in res.matches {
                        if !self.result_map.contains_key(&tag) {
                            self.result_map.insert(tag.clone(), vec![]);
                        }

                        let result = self.result_map.get_mut(&tag).expect("Could not get result map");

                        for mut search_match in matches {
                            let line = search_match.line as i32 + offset;
                            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
                            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
                            buffer.apply_tag_by_name(&tag, &iter_start, &iter_end);

                            search_match.line = line as usize;
                            result.push(search_match);
                        }
                    }
                }
            }
            LogTextViewMsg::ScrollToBookmark(key) => {
                if let Some((line, _)) = self.marker.get(&key) {
                    if let Some(buffer) = self.text_view.get_buffer() {
                        let mut iter = buffer.get_iter_at_line(*line);
                        self.text_view.scroll_to_iter(&mut iter, 0.0, true, 0.5, 0.5);
                    }
                }
            }
            LogTextViewMsg::ToggleBookmark(key) => {
                if let Some(buffer) = self.text_view.get_buffer() {
                    let buffer = buffer.downcast::<sourceview::Buffer>().unwrap();
                    let cursor_pos = buffer.get_property_cursor_position();
                    let cursor_pos = buffer.get_iter_at_offset(cursor_pos);
                    let cursor_line = cursor_pos.get_line();
                    let line_pos = buffer.get_iter_at_line(cursor_line);
                    let mut add_marker = true;
                    if let Some((marker_line, existing)) = self.marker.remove(&key) {
                        buffer.delete_mark(&existing);
                        add_marker = marker_line != cursor_line;
                    }

                    if add_marker {
                        let mark = buffer.create_source_mark(None, MARKER_CATEGORY_BOOKMARK, &line_pos).unwrap();
                        self.marker.insert(key, (cursor_line, mark));
                    }

                    self.text_view.set_show_line_marks(self.marker.len() > 0);
                }
            }
            LogTextViewMsg::CursorChanged => {

                if let Some(buffer) = self.text_view.get_buffer() {
                    let cursor_pos = buffer.get_property_cursor_position();
                    let cursor_pos = buffer.get_iter_at_offset(cursor_pos);

                    self.result_match_cursor_pos = self.get_next_from_pos(&cursor_pos, &self.active_rule);
                }
            }
            LogTextViewMsg::Clear => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    buffer.set_text("");
                    self.clear_result_selection_data();
                }
            }
        }
    }


    fn get_next_from_pos(&self, current_pos: &TextIter, rule_id: &String) -> Option<usize> {
        if let Some(buffer) = self.text_view.get_buffer() {
            if let Some(search_results) = self.result_map.get(rule_id) {
                let next = search_results.iter().enumerate().find(|(_, m)| {
                    let search_match_pos = buffer.get_iter_at_line_index(m.line as i32, m.start as i32);
                    &search_match_pos > current_pos
                }).map(|(idx, _)| idx).or_else(|| {
                    search_results.iter().enumerate().last().map(|(idx, _)| idx)
                });

                if let Some(next) = next {
                    let next = if next > 0 { next } else { 0 };
                    return Some(next);
                }
            }
        }
        return None
    }

    fn clear_result_selection_data(&mut self) {
        for (id, pos) in &self.current_result_selection {
            if let Some(d) = self.result_map.get(id).and_then(|m| m.get(*pos)) {
                Self::set_or_remove_selected_match(&self.text_view, &d, false);
            }
        }
        self.current_result_selection.take();
        self.result_map.clear();
    }

    pub fn clear_log(&mut self) {
        if let Some(buffer) = self.text_view.get_buffer() {
            self.result_map.clear();
            self.current_result_selection.take();
            buffer.set_text("");
        }
    }

    pub fn apply_rules(&mut self, mut rules: Vec<Highlighter>) {
        let mut add = vec![];
        let mut remove = vec![];
        let mut update = vec![];

        rules.sort_by_key(|i| i.id);
        let init = self.rules.len() <= 0;
        let mut clear_cursor = false;
        let mut has_changes = false;
        let compare_results = SortedListCompare::new(&mut self.rules, &mut rules);
        for compare_result in compare_results {
            let text_view = self.text_view.clone();
            match compare_result {
                CompareResult::MissesLeft(new) => {
                    has_changes = true;
                    add.push(new.clone());
                    if let Some(tags) = text_view.get_buffer()
                        .and_then(|buffer| buffer.get_tag_table()) {
                        let tag = TextTag::new(Some(&new.id.to_string()));
                        tag.set_property_background(new.color.as_ref().map(|c|c.as_str()));
                        tags.add(&tag);
                        if new.is_system {
                            tag.set_priority(tags.get_size() - 2);
                        }else {
                            tag.set_priority(0);
                        }

                    }
                }
                CompareResult::MissesRight(delete) => {
                    has_changes = true;
                    remove.push(delete.id.to_string());
                    if let Some(tags) = text_view.get_buffer().and_then(|buffer| buffer.get_tag_table()) {
                        if let Some(tag) = tags.lookup(&delete.id.to_string()) {
                            tags.remove(&tag);
                        }
                    }
                }
                CompareResult::Equal(left, right) => {
                    if let Some(tag) = text_view.get_buffer()
                        .and_then(|buffer| buffer.get_tag_table())
                        .and_then(|tag_table| tag_table.lookup(&left.id.to_string())) {
                        tag.set_property_background(right.color.as_ref().map(|s|s.as_str()));
                    }
                    if left.regex != right.regex || left.is_exclude() != right.is_exclude() {
                        has_changes = true;
                        update.push(right.clone());
                        if let Some(tb) = text_view.get_buffer() {
                            let (start, end) = tb.get_bounds();
                            tb.remove_tag_by_name(&left.id.to_string(), &start, &end);
                        }
                        clear_cursor = true;
                    }
                }
            }
        }
        if clear_cursor {
            self.clear_result_selection_data();
        }

        if has_changes {
            let mut data :Option<String> = None;
            if !init {
                let text_view = self.text_view.clone();
                if let Some(tb) = text_view.get_buffer() {
                    let (start, end) = tb.get_bounds();
                    data = tb.get_text(&start, &end, false).map(|s|s.to_string());
                }
            }
            if let Some(thread_action_sender) = self.thread_action_sender.as_ref() {
                thread_action_sender.send(LogTextViewThreadMsg::ApplyRules(RuleChanges {
                    add,
                    remove,
                    update,
                    data,
                })).expect("Could not send apply rules");
            }
        }
        self.rules = rules;
    }

    pub fn toggle_autoscroll(&mut self, enable: bool) {
        if enable {
            self.enable_auto_scroll();
        } else {
            self.disable_auto_scroll();
        }
    }

    pub fn close(&mut self) {
        if let Some(sender) = self.thread_action_sender.as_ref() {
            if let Err(e) = sender.send(LogTextViewThreadMsg::Quit) {
                error!("Could not send quit msg: {}", e);
            }
        }
    }

    pub fn enable_auto_scroll(&mut self) {
        let handler = enable_auto_scroll(&*self.text_view);
        self.autoscroll_handler = Some(handler);
    }

    pub fn disable_auto_scroll(&mut self) {
        if let Some(handler) = self.autoscroll_handler.take() {
            let text_view = &*self.text_view;
            text_view.disconnect(handler);
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}



fn register_log_data_watcher<T>(sender: T, mut log_reader: Box<dyn LogReader>, rx: std::sync::mpsc::Receiver<LogTextViewThreadMsg>)
    where T : 'static + Send + Clone + Fn(LogTextViewMsg)
{
    tokio::task::spawn(async move {
        let mut active_rules = vec![];

        let mut encoding = None;
        let replacers = vec![
            LogReplacer { regex: Regex::new(r"\\n\\r|\\r\\n|\\r").unwrap(), replace_with: "\n" },
            LogReplacer { regex: Regex::new(r"|\\0").unwrap(), replace_with: "" }
        ];

        loop {
            log_reader.init().await;

            let check_changes = match log_reader.check_changes() {
                LogState::Skip => false,
                LogState::Ok => true,
                LogState::Reload => {
                    sender(LogTextViewMsg::Clear);
                    false
                }
            };

            if check_changes {
                let mut full_search_data = None;
                if let Ok(msg) = rx.try_recv() {
                    match msg {
                        LogTextViewThreadMsg::ApplyRules(changes) => {
                            full_search_data = changes.data;
                            for new in &changes.add {
                                let regex = new.regex.as_ref().and_then(|r| Regex::new(r).ok());
                                active_rules.push(ActiveRule {
                                    id: new.id.to_string(),
                                    line_offset: 0,
                                    regex,
                                    is_exclude: new.is_exclude()
                                });
                            }
                            for remove in &changes.remove {
                                if let Some((idx, _item)) = active_rules.iter().enumerate().find(|(_, e)| &e.id == remove) {
                                    active_rules.remove(idx);
                                }
                            }

                            for update in &changes.update {
                                let sid = update.id.to_string();
                                if let Some((_idx, search)) = active_rules.iter_mut().enumerate().find(|(_, item)| item.id == sid) {
                                    if let Some(regex) = update.regex.as_ref() {
                                        search.line_offset = 0;
                                        search.regex = Regex::new(regex).ok();
                                        search.is_exclude = update.is_exclude()
                                    } else {
                                        search.regex.take();
                                    }
                                }
                            }
                        }
                        LogTextViewThreadMsg::Quit => {
                            debug!("Quit signal");
                            break;
                        }
                    }
                }

                if let Some(data) = full_search_data {
                    if let Ok(result) = search(data, &mut active_rules, true) {
                        if result.matches.len() > 0 {
                            sender(LogTextViewMsg::Data(result));
                        }
                    }
                } else {
                    if let Ok(data) = log_reader.read().await {
                        let read_bytes = data.len();
                        if let Ok(data) = decode_data(&data, &mut encoding, &replacers) {
                            if let Ok(result) = search(data, &mut active_rules, false) {
                                if read_bytes > 0 {
                                    sender(LogTextViewMsg::Data(result));
                                }
                            }
                        }
                    }
                }
            }

            tokio::time::delay_for(std::time::Duration::from_millis(500)).await;
        }
        log_reader.stop();
        info!("File watcher stopped");
    });
}