use gtk::{prelude::*, TreeStore, ScrolledWindowBuilder, TreeIter, SortType, SortColumn, TreeView, IconSize, ReliefStyle, AccelGroup, AccelFlags};
use gtk::{ScrolledWindow, Orientation, TextTag, TextTagTable};
use std::rc::Rc;
use glib::{SignalHandlerId};
use crate::util::{enable_auto_scroll, SortedListCompare, CompareResult, search, decode_data, add_css_with_name, create_col, ColumnType};
use crate::model::{LogTextViewMsg, LogViewData, LogReplacer, ExtractSelection};
use crate::log_text_contrast::{matching_foreground_color_for_background};

use sourceview::{ViewExt, BufferExt, Mark};
use regex::Regex;
use std::collections::{HashMap};
use crate::highlighters::{Highlighter};

use log::{info, error, debug};
use crate::log_file_reader::LogFileReader;
use crate::kubernetes_log_reader::KubernetesLogReader;
use crate::model::{LogState, ActiveRule, LogReader, LogTextViewThreadMsg, RuleChanges};

const EXTRACT_TYPE_GROUP : &'static u8 = &0;
const EXTRACT_TYPE_CHILD : &'static u8 = &1;

const EXTRACT_COL_TYPE :  u32 = 0;
const EXTRACT_COL_SEARCH_ID : u32 = 1;
const EXTRACT_COL_CHECKSUM : u32 = 2;
const EXTRACT_COL_NAME : u32 = 3;
const EXTRACT_COL_COUNT : u32 = 4;
const EXTRACT_COL_TEXT : u32 = 5;
const EXTRACT_COL_LINE : u32 = 6;

enum Step {
    First,
    Forward,
    Backward,
}

struct ExtractData {
    count: i32,
    item: TreeIter,
    positions: Vec<i32>,
}

struct SearchGroup {
    count: i32,
    item: TreeIter,
    positions: Vec<i32>,
    children: HashMap<u32, ExtractData>
}

impl SearchGroup {
    fn new(item: TreeIter) -> Self {
        Self {
            count: 0,
            item,
            positions: vec![],
            children: HashMap::new()
        }
    }
}

pub struct LogTextView {
    container: gtk::Paned,
    text_view: Rc<sourceview::View>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<Highlighter>,
    thread_action_sender: Option<std::sync::mpsc::Sender<LogTextViewThreadMsg>>,
    bookmarks: HashMap<u16, (i32, Mark)>,
    extracted_data_model: Rc<TreeStore>,
    extracted_data_view: TreeView,
    extract: HashMap<String, SearchGroup>,
    active_extract: Option<(ExtractSelection, usize)>,
    active_line_mark: Option<Mark>,
}

const MARKER_CATEGORY_BOOKMARK: &'static str = "BOOKMARK";
const MARKER_CATEGORY_LINE: &'static str = "LINE";

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

        {
            let tx = sender.clone();
            self.extracted_data_view.connect_row_activated(move |tv, _path, _col| {
                if let Some((item, iter)) = tv.get_selection().get_selected() {
                    if let Some(col_type) = item.get_value(&iter, EXTRACT_COL_TYPE as i32).get::<u8>().unwrap_or(None) {
                        let selection = if &col_type == EXTRACT_TYPE_GROUP {
                            item.get_value(&iter, EXTRACT_COL_SEARCH_ID as i32).get::<String>().unwrap_or(None).map(|s| ExtractSelection::SearchGroup(s))
                        }else {
                            let search_id = item.get_value(&iter, EXTRACT_COL_SEARCH_ID as i32).get::<String>().unwrap_or(None).expect("Invalid item without search id");
                            item.get_value(&iter, EXTRACT_COL_CHECKSUM as i32).get::<u32>().unwrap_or(None).map(|s| ExtractSelection::TextGroup(search_id, s))
                        };
                        if let Some(r) = selection {
                            tx(LogTextViewMsg::ExtractSelected(r));
                        }
                    }
                }
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


    pub fn new<T>(accelerators: &AccelGroup, tx: T) -> Self
        where T : 'static + Send + Clone + Fn(LogTextViewMsg)
    {
        let tag_table = TextTagTable::new();
        let text_buffer = sourceview::Buffer::new(Some(&tag_table));
        let tv = sourceview::View::new_with_buffer(&text_buffer);
        add_css_with_name(&tv, "textview", r##"
            #textview {
                  font: 12px "Monospace";
            }
        "##);

        let bookmark_marker = sourceview::MarkAttributesBuilder::new()
            .icon_name("radio-symbolic")
            .build();
        tv.set_mark_attributes(MARKER_CATEGORY_BOOKMARK, &bookmark_marker, 10);

        let current_line_marker = sourceview::MarkAttributesBuilder::new()
            .icon_name("pan-end-symbolic")
            .build();
        tv.set_mark_attributes(MARKER_CATEGORY_LINE, &current_line_marker, 10);

        tv.set_editable(false);
        tv.set_show_line_numbers(true);
        tv.set_show_line_marks(true);
        tv.set_child_visible(true);

        let minimap = sourceview::MapBuilder::new()
            .vexpand_set(true)
            .view(&tv)
            .width_request(220)
            .buffer(&text_buffer)
            .highlight_current_line(true)
            .build();

        add_css_with_name(&minimap, "minimap",r##"
            #minimap {
                  font: 1px "Monospace";
                  background-color: rgba(0,0,0,0);
            }
        "##);

        let text_view = Rc::new(tv);
        let scroll_wnd = ScrolledWindow::new(text_view.get_hadjustment().as_ref(), text_view.get_vadjustment().as_ref());
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_hexpand(true);
        scroll_wnd.add(&*text_view);

        let paned = gtk::Paned::new(Orientation::Vertical);
        let extracted_data_model = Rc::new(gtk::TreeStore::new(&[
            glib::Type::U8,         // Type             0
            glib::Type::String,     // SearchId         1
            glib::Type::U32,        // Checksum         2
            glib::Type::String,     // Name             3
            glib::Type::I32,        // Count            4
            glib::Type::String,     // Extracted Text   5
            glib::Type::I32,        // Line             6
        ]));
        let extracted_data_view = gtk::TreeView::with_model(&*extracted_data_model);
        extracted_data_view.set_activate_on_single_click(true);
        extracted_data_model.set_sort_column_id(SortColumn::Index(EXTRACT_COL_LINE), SortType::Descending);
        let extract_scroll_view = ScrolledWindowBuilder::new()
            .expand(true)
            .height_request(100)
            .build();

        let ed = gtk::BoxBuilder::new()
            .orientation(Orientation::Horizontal)
            .build();


        let result_toolbar = Self::build_results_toolbar(tx.clone(), accelerators);

        ed.add(&result_toolbar);
        ed.add(&extract_scroll_view);

        extract_scroll_view.add(&extracted_data_view);
        extracted_data_view.append_column(&create_col(Some("Rule"), EXTRACT_COL_NAME as i32, ColumnType::String, extracted_data_model.clone()));
        extracted_data_view.append_column(&create_col(Some("Matches"), EXTRACT_COL_COUNT as i32, ColumnType::Number, extracted_data_model.clone()));
        extracted_data_view.append_column(&create_col(Some("Last matching line"), EXTRACT_COL_LINE as i32, ColumnType::Number, extracted_data_model.clone()));
        extracted_data_view.append_column(&create_col(Some("Extracted"), EXTRACT_COL_TEXT as i32, ColumnType::String, extracted_data_model.clone()));

        let text_container = gtk::BoxBuilder::new()
            .orientation(Orientation::Horizontal)
            .height_request(350)
            .build();
        text_container.add(&scroll_wnd);
        text_container.add(&minimap);

        paned.pack1(&text_container, true, false);
        paned.pack2(&ed, false, false);

        Self {
            container: paned,
            text_view,
            autoscroll_handler: None,
            rules: vec![],
            thread_action_sender: None,
            bookmarks: HashMap::new(),
            extracted_data_model,
            extract: HashMap::new(),
            extracted_data_view,
            active_extract: None,
            active_line_mark: None,
        }
    }

    fn build_results_toolbar<T>(tx: T, accelerators: &AccelGroup) -> gtk::Box
        where T : 'static + Send + Clone + Fn(LogTextViewMsg)
    {
        let sidebar = gtk::BoxBuilder::new()
            .orientation(Orientation::Vertical)
            .width_request(20)
            .build();

        add_css_with_name(&sidebar, "toolbar", r"
            #toolbar {
                background-color: rgba(248,248,248,255);
                border-right: 1px solid #c0c0c0;
            }
        ");

        let tb_header = gtk::BoxBuilder::new()
            .orientation(Orientation::Horizontal)
            .height_request(31)
            .build();
        add_css_with_name(&tb_header, "toolbar", r"
            #toolbar {
                background-color: rgba(237,237,237,255);
                border-bottom: 1px solid #c0c0c0;
            }
        ");
        sidebar.add(&tb_header);

        let btn_toolbar = gtk::BoxBuilder::new()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin(2)
            .build();
        sidebar.add(&btn_toolbar);

        let btn = gtk::ButtonBuilder::new()
            .image(&gtk::Image::from_icon_name(Some("pan-up-symbolic"), IconSize::Menu))
            .height_request(16)
            .width_request(16)
            .relief(ReliefStyle::None)
            .build();
        {
            let (key, modifier) = gtk::accelerator_parse("<Primary>P");
            btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);

            let tx = tx.clone();
            btn.connect_clicked(move |_| {
                tx(LogTextViewMsg::PrevMatch);
            });
        }
        btn_toolbar.add(&btn);

        let btn = gtk::ButtonBuilder::new()
            .image(&gtk::Image::from_icon_name(Some("pan-down-symbolic"), IconSize::Menu))
            .height_request(16)
            .width_request(16)
            .relief(ReliefStyle::None)
            .build();
        {
            let (key, modifier) = gtk::accelerator_parse("<Primary>N");
            btn.add_accelerator("activate", accelerators, key, modifier, AccelFlags::VISIBLE);
            let tx = tx.clone();
            btn.connect_clicked(move |_| {
                tx(LogTextViewMsg::NextMatch);
            });
        }
        btn_toolbar.add(&btn);
        sidebar
    }

    fn scroll_to_line(&self, line: i32) {
        let text_view = self.text_view.clone();
        glib::idle_add_local(move || {
            if let Some(buffer) = text_view.get_buffer() {
                let mut iter = buffer.get_iter_at_line(line);
                let iter_loc = text_view.get_iter_location(&iter);
                let visible_rect = text_view.get_visible_rect();

                text_view.scroll_to_iter(&mut iter, 0.0, true, 0.5, 0.5);
                if visible_rect.intersect(&iter_loc).is_none() {
                    glib::Continue(true)
                } else {
                    glib::Continue(false)
                }
            }else {
                glib::Continue(false)
            }
        });
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

                    for (search_id, search_result_data) in res.rule_search_result {
                        let name = &search_result_data.name.expect("Search result data should have a name");
                        if !self.extract.contains_key(&search_id) {
                            let item = self.extracted_data_model.insert_with_values(None, None,
                            &[EXTRACT_COL_TYPE, EXTRACT_COL_SEARCH_ID, EXTRACT_COL_NAME, EXTRACT_COL_COUNT],
                            &[EXTRACT_TYPE_GROUP, &search_id, name, &0]);
                            self.extract.insert(search_id.clone(), SearchGroup::new(item));
                        }

                        let mut extract_group = self.extract.get_mut(&search_id).unwrap();

                        for search_match in search_result_data.matches {
                            let line = search_match.line as i32 + offset;
                            extract_group.positions.push(line);
                            extract_group.count += 1;
                            self.extracted_data_model.set(&extract_group.item, &[EXTRACT_COL_COUNT, EXTRACT_COL_LINE], &[&extract_group.count, &(line + 1)]);

                            if let Some(extracted_text) = &search_match.extracted_text {
                                let text_id = crc::crc32::checksum_ieee(extracted_text.as_bytes());
                                if let Some(extract) = extract_group.children.get_mut(&text_id) {
                                    extract.count += 1;
                                    extract.positions.push(line);
                                    self.extracted_data_model.set(&extract.item, &[EXTRACT_COL_COUNT, EXTRACT_COL_LINE], &[&extract.count, &(line + 1)]);
                                } else {
                                    let child = self.extracted_data_model.insert_with_values(
                                        Some(&extract_group.item), None,
                                        &[EXTRACT_COL_TYPE, EXTRACT_COL_SEARCH_ID, EXTRACT_COL_CHECKSUM, EXTRACT_COL_COUNT, EXTRACT_COL_TEXT, EXTRACT_COL_LINE],
                                        &[EXTRACT_TYPE_CHILD, &search_id ,&text_id, &1, extracted_text, &(line + 1)]);

                                    extract_group.children.insert(text_id, ExtractData {
                                        count: 1,
                                        item: child,
                                        positions: vec![line]
                                    });
                                }
                            }

                            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
                            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
                            buffer.apply_tag_by_name(&search_id, &iter_start, &iter_end);
                        }
                    }
                }
            }
            LogTextViewMsg::ScrollToBookmark(key) => {
                if let Some((line, _)) = self.bookmarks.get(&key) {
                    self.scroll_to_line(*line);
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
                    if let Some((marker_line, existing)) = self.bookmarks.remove(&key) {
                        buffer.delete_mark(&existing);
                        add_marker = marker_line != cursor_line;
                    }

                    if add_marker {
                        let mark = buffer.create_source_mark(None, MARKER_CATEGORY_BOOKMARK, &line_pos).unwrap();
                        self.bookmarks.insert(key, (cursor_line, mark));
                    }
                }
            }
            LogTextViewMsg::CursorChanged => {
                if let Some(_buffer) = self.text_view.get_buffer() {
                    // let cursor_pos = buffer.get_property_cursor_position();
                    // let cursor_pos = buffer.get_iter_at_offset(cursor_pos);
                    //
                }
            }
            LogTextViewMsg::Clear => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    buffer.set_text("");
                }
            }
            LogTextViewMsg::ExtractSelected(selection) => {
                if let Some(active) = self.active_line_mark.take() {
                    if let Some(buffer) = self.text_view.get_buffer() {
                        let buffer = buffer.downcast::<sourceview::Buffer>().unwrap();
                        buffer.delete_mark(&active);
                    }
                }
                self.select_match(Step::First, selection, 0);
            }
            LogTextViewMsg::NextMatch => {
                self.select_next_match();
            }
            LogTextViewMsg::PrevMatch => {
                self.select_prev_match();
            }
        }
    }

    pub fn select_prev_match(&mut self) {
        if let Some((sel, pos)) = self.active_extract.take() {
            self.select_match(Step::Backward, sel, pos);
        }
    }

    pub fn select_next_match(&mut self) {
        if let Some((sel, pos)) = self.active_extract.take() {
            self.select_match(Step::Forward, sel, pos);
        }
    }

    fn select_match(&mut self, step: Step, sel: ExtractSelection, pos: usize) {
        let lines = match &sel {
            ExtractSelection::SearchGroup(search_id) => {
                self.extract.get(search_id).and_then(|search_group|Some(&search_group.positions))
            }
            ExtractSelection::TextGroup(search_id, text_id) => {
                self.extract.get(search_id)
                    .and_then(|search_group| search_group.children.get(text_id))
                    .and_then(|text_group|Some(&text_group.positions))
            }
        };

        if let Some(lines) = lines {
            let next_pos = self.get_next_pos(lines, pos, step);
            if let Some(buffer) = self.text_view.get_buffer() {
                let buffer = buffer.downcast::<sourceview::Buffer>().unwrap();
                if let Some(prev_mark) = self.active_line_mark.take() {
                    buffer.delete_mark(&prev_mark);
                }

                if let Some(next_line) = lines.get(next_pos) {
                    self.scroll_to_line(*next_line);
                    let next = buffer.get_iter_at_line(*next_line);
                    let mark = buffer.create_source_mark(None, MARKER_CATEGORY_LINE, &next).unwrap();
                    self.active_line_mark = Some(mark);
                }
            }

            self.active_extract = Some((sel, next_pos));
        }
    }

    fn get_next_pos(&self, positions: &Vec<i32>, pos: usize, step: Step) -> usize {
        match step {
            Step::First => {
              if positions.len() > 0 { positions.len() -1 } else { 0 }
            }
            Step::Forward => {
                if pos == positions.len() -1 { 0 }else { pos + 1 }
            }
            Step::Backward => {
                if pos > 0 { pos - 1 } else { positions.len() -1 }
            }
        }
    }

    pub fn clear_log(&mut self) {
        if let Some(buffer) = self.text_view.get_buffer() {
            buffer.set_text("");
        }
    }
    
    pub fn add_separator_line(&mut self) {
        if let Some(buffer) = &self.text_view.get_buffer() {
            let (_start, mut end) = buffer.get_bounds();            
            buffer.insert(&mut end, "-------------------------------------------------------------------------------\n");
        }
    }

    pub fn apply_rules(&mut self, mut rules: Vec<Highlighter>) {
        let mut add = vec![];
        let mut remove = vec![];
        let mut update = vec![];

        rules.sort_by_key(|i| i.id);
        let init = self.rules.len() <= 0;
        let mut has_changes = false;
        let mut clear_data = false;
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
                        let background = tag.get_property_background_rgba();                
                        tag.set_property_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());
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
                        let background = tag.get_property_background_rgba();                
                        tag.set_property_foreground_rgba(matching_foreground_color_for_background(&background).as_ref());
                    }
                    if left.regex != right.regex
                        || left.is_exclude() != right.is_exclude()
                        || left.extractor_regex != right.extractor_regex {
                        has_changes = true;
                        clear_data = true;
                        let id = right.id.to_string();
                        if let Some(data) = self.extract.get(&id) {
                            self.extracted_data_model.remove(&data.item);
                            self.extract.remove(&id);
                        }

                        update.push(right.clone());
                        if let Some(tb) = text_view.get_buffer() {
                            let (start, end) = tb.get_bounds();
                            tb.remove_tag_by_name(&left.id.to_string(), &start, &end);
                        }
                    }
                }
            }
        }

        if clear_data {
            self.active_extract = None;
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

    pub fn view(&self) -> &gtk::Paned {
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
                                    name: new.name.clone(),
                                    is_exclude: new.is_exclude(),
                                    is_dirty: false,
                                    extractor_regex: new.extractor_regex.as_ref().and_then(|r| Regex::new(r).ok())
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
                                        search.is_exclude = update.is_exclude();
                                        search.is_dirty = true;
                                        if let Some(ex) = update.extractor_regex.as_ref() {
                                            search.extractor_regex = Regex::new(ex).ok();
                                        }
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
                        if result.rule_search_result.len() > 0 {
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