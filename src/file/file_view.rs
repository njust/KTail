use gtk::{prelude::*, TextIter, TextBuffer};

use gtk::{ScrolledWindow, Orientation, TextTag, TextTagTable};
use std::rc::Rc;
use std::path::PathBuf;
use glib::{SignalHandlerId};
use crate::util::{enable_auto_scroll, SortedListCompare, CompareResult, CREATE_NO_WINDOW, search, decode_data, read};
use crate::{FileViewMsg, FileViewData, SearchResultMatch, CreateKubeLogData};
use crate::file::{FileThreadMsg, Rule, RuleChanges, ActiveRule};
use sourceview::{ViewExt};
use regex::Regex;
use std::collections::HashMap;
use crate::rules::SEARCH_ID;
use std::io::{Seek, SeekFrom};
use std::error::Error;
use std::process::{Child};
use k8s_client::{KubeClient, LogOptions};
use stream_cancel::{Valved, Trigger};
use tokio::sync::oneshot::Sender;
use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;
use log::{info, error, debug};

pub struct FileView {
    container: gtk::Box,
    text_view: Rc<sourceview::View>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<Rule>,
    active_rule: String,
    thread_action_sender: Option<std::sync::mpsc::Sender<FileThreadMsg>>,
    result_map: HashMap<String, Vec<SearchResultMatch>>,
    current_result_selection: Option<(String, usize)>,
    result_match_cursor_pos: Option<usize>,
    child_process: Option<Child>,
}

#[async_trait]
pub trait LogReader : std::marker::Send {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn init(&mut self);
    fn check_changes(&mut self) -> LogState;
    fn stop(&mut self);
}

pub struct LogFileReader {
    path: PathBuf,
    file: std::fs::File,
    offset: u64,
}

impl LogFileReader {
    pub fn new(path: PathBuf) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(&path)?;
        Ok(Self {
            path,
            file,
            offset: 0
        })
    }
}

pub enum LogState {
    Ok,
    Skip,
    Reload
}

#[async_trait]
impl LogReader for LogFileReader {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        if self.offset > 0 {
            self.file.seek(SeekFrom::Start(self.offset))?;
        }
        let read = read(&mut self.file)?;
        self.offset += read.len() as u64;
        Ok(read)
    }

    async fn init(&mut self) {
    }

    fn check_changes(&mut self) -> LogState {
        if !self.path.exists() {
            return LogState::Skip;
        }

        if let Ok(metadata) = std::fs::metadata(&self.path) {
            let len = metadata.len();
            if len <= 0 {
                return LogState::Skip;
            }
            if len < self.offset {
                self.offset = 0;
                return LogState::Reload;
            }
        }

        return LogState::Ok;
    }

    fn stop(&mut self) {
    }
}

pub struct KubernetesLogReader {
    options: CreateKubeLogData,
    is_initialized: bool,
    is_stopping: bool,
    data_rx: Option<Receiver<KubernetesLogReaderMsg>>,
    data_tx: Option<tokio::sync::mpsc::Sender<KubernetesLogReaderMsg>>,
    streams: HashMap<String, (Sender<Trigger>, Trigger)>
}

pub enum KubernetesLogReaderMsg {
    Data(Vec<u8>),
    ReInit(String),
}

#[async_trait]
impl LogReader for KubernetesLogReader {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut r = vec![];
        loop {
            if let Some(rx) = self.data_rx.as_mut() {
                if let Ok(rc) = rx.try_recv() {
                    match rc {
                        KubernetesLogReaderMsg::Data(mut data) => {
                            if data.len() > 0 {
                                r.append(&mut data)
                            }else {
                                break;
                            }
                        }
                        KubernetesLogReaderMsg::ReInit(pod) => {
                            self.is_initialized = false;
                            self.streams.remove(&pod);
                            break;
                        }
                    }
                }else {
                    break;
                }
            }
        }
        Ok(r)
    }

    async fn init(&mut self) {
        use tokio::stream::StreamExt;
        if self.is_initialized || self.is_stopping {
            return;
        }
        self.is_initialized = true;

        let c = KubeClient::load_conf(None).unwrap();
        let mut pod_list = vec![];

        if let Ok(pods) = c.pods().await {
            for pod in pods {
                if let Some(name) = pod.metadata.name {
                    for pod_name in &self.options.pods {
                        if name.starts_with(pod_name) {
                            if let Some(container_status) = pod.status.as_ref().and_then(|s|s.container_statuses.as_ref()).and_then(|cs|cs.first()) {
                                if container_status.ready {
                                    pod_list.push(name.clone());
                                }else {
                                    self.is_initialized = false;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        for pod in pod_list {
            if self.streams.contains_key(&pod) {
                // info!("Skipping initiate stream for pod '{}'", pod);
                continue;
            }

            if let Ok(log_stream) = c.logs(&pod, Some(
                LogOptions {
                    follow: Some(true),
                    since_seconds: Some(3600 * self.options.since),
                }
            )).await {
                let (exit_tx, _exit_rx) = tokio::sync::oneshot::channel::<stream_cancel::Trigger>();
                let (exit, mut inc) = Valved::new(log_stream);
                self.streams.insert(pod.clone(), (exit_tx, exit));
                let mut tx = self.data_tx.clone().unwrap();
                let pod_name = pod.clone();
                tokio::spawn(async move {
                    info!("Stream for pod '{}' started", pod_name);
                    while let Some(Ok(res)) = inc.next().await {
                        let data = res.to_vec();
                        // if data.starts_with(b"unable to retrieve container logs") || data.starts_with(b"rpc error: ") {
                        //     if let Ok(data) = String::from_utf8(data) {
                        //         error!("Kublet error: {}", data);
                        //     }
                        // }else {
                        //     if let Err(e) = tx.send(KubernetesLogReaderMsg::Data(data)).await {
                        //         error!("Failed to send stream data for pod '{}': {}", pod_name, e);
                        //     }
                        // }
                        if let Err(e) = tx.send(KubernetesLogReaderMsg::Data(data)).await {
                            error!("Failed to send stream data for pod '{}': {}", pod_name, e);
                        }
                    }
                    info!("Stream for pod '{}' ended", pod_name);
                    if let Err(e) = tx.send(KubernetesLogReaderMsg::ReInit(pod_name)).await {
                        debug!("Could not send kubernetes re init msg: {}", e);
                    }
                });
            }
        }
    }

    fn check_changes(&mut self) -> LogState {
        LogState::Ok
    }

    fn stop(&mut self) {
        self.is_stopping = true;
        let pods = self.streams.keys().map(|s|s.clone()).collect::<Vec<String>>();
        for p in pods {
            if let Some((sender, trigger)) = self.streams.remove(&p) {
                if let Err(e) = sender.send(trigger) {
                    debug!("Could not send exit trigger: {:?}", e);
                }
            }
        }
    }
}

impl KubernetesLogReader {
    pub fn new(data: CreateKubeLogData) -> Self {
        let (data_tx, data_rx) = tokio::sync::mpsc::channel::<KubernetesLogReaderMsg>(10000);
        let mut instance = Self {
            data_rx: Some(data_rx),
            data_tx: Some(data_tx),
            options: data,
            is_initialized: false,
            is_stopping: false,
            streams: HashMap::new(),
        };
        Self::init(&mut instance);
        instance
    }
}

const CURRENT_CURSOR_TAG: &'static str = "CURRENT_CURSOR";

impl FileView {
    pub fn start<T>(&mut self, data: FileViewData, sender: T, default_rules: Vec<Rule>)
        where T : 'static + Send + Clone + Fn(FileViewMsg)
    {
        let (thread_action_sender, thread_action_receiver) =
            std::sync::mpsc::channel::<FileThreadMsg>();

        self.thread_action_sender = Some(thread_action_sender);
        self.apply_rules(default_rules);

        {
            let tx = sender.clone();
            self.text_view.connect_button_press_event(move |_,_|{
                tx(FileViewMsg::CursorChanged);
                gtk::Inhibit(false)
            });
        }

        {
            let tx = sender.clone();
            self.text_view.connect_move_cursor(move |_,_,_,_| {
                tx(FileViewMsg::CursorChanged);
            });
        }

        let file_thread_tx = sender.clone();
        match data {
            FileViewData::File(path) => {
                let reader = LogFileReader::new(path).unwrap();
                register_file_watcher_thread(move |msg| {
                    file_thread_tx(msg);
                }, Box::new(reader),  thread_action_receiver);
            }
            FileViewData::Kube(data) => {
                let reader = KubernetesLogReader::new(data);
                register_file_watcher_thread(move |msg| {
                    file_thread_tx(msg);
                }, Box::new(reader),  thread_action_receiver);
            }
        }
    }
    pub fn new() -> Self {
        let tag_table = TextTagTable::new();
        let current_cursor_tag = gtk::TextTag::new(Some(CURRENT_CURSOR_TAG));
        current_cursor_tag.set_property_background(Some("rgba(114,159,207,1)"));
        tag_table.add(&current_cursor_tag);
        current_cursor_tag.set_priority(tag_table.get_size() - 1);

        let text_buffer = sourceview::Buffer::new(Some(&tag_table));
        let tv = sourceview::View::new_with_buffer(&text_buffer);
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
            child_process: None,
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

    pub fn update(&mut self, msg: FileViewMsg) {
        match msg {
            FileViewMsg::Data(read, data, search_result_list) => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    let (_start, mut end) = buffer.get_bounds();
                    if read > 0 {
                        buffer.insert(&mut end, &data);
                    }
                    for (tag, matches) in search_result_list {
                        if !self.result_map.contains_key(&tag) {
                            self.result_map.insert(tag.clone(), vec![]);
                        }

                        let result = self.result_map.get_mut(&tag).expect("Could not get result map");

                        for search_match in matches {
                            let line = search_match.line as i32;
                            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
                            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
                            buffer.apply_tag_by_name(&tag, &iter_start, &iter_end);

                            result.push(search_match);
                        }
                    }
                }
            }
            FileViewMsg::CursorChanged => {
                if let Some(buffer) = self.text_view.get_buffer() {
                    let cursor_pos = buffer.get_property_cursor_position();
                    let cursor_pos = buffer.get_iter_at_offset(cursor_pos);
                    self.result_match_cursor_pos = self.get_next_from_pos(&cursor_pos, &self.active_rule);
                }
            }
            FileViewMsg::Clear => {
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

    pub fn apply_rules(&mut self, mut rules: Vec<Rule>) {
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
                    if left.regex != right.regex {
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
                thread_action_sender.send(FileThreadMsg::ApplyRules(RuleChanges {
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
            sender.send(FileThreadMsg::Quit).expect("Could not send quit msg");
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

fn register_file_watcher_thread<T>(sender: T, mut log_reader: Box<dyn LogReader>, rx: std::sync::mpsc::Receiver<FileThreadMsg>)
    where T : 'static + Send + Clone + Fn(FileViewMsg)
{
    tokio::task::spawn(async move {
        let mut line_offset = 0;
        let mut active_rules = vec![];

        loop {
            log_reader.init().await;

            let check_changes = match log_reader.check_changes() {
                LogState::Skip => false,
                LogState::Ok => true,
                LogState::Reload => {
                    line_offset = 0;
                    sender(FileViewMsg::Clear);
                    false
                }
            };

            if check_changes {
                let mut full_search_data = None;
                if let Ok(msg) = rx.try_recv() {
                    match msg {
                        FileThreadMsg::ApplyRules(changes) => {
                            full_search_data = changes.data;
                            for new in &changes.add {
                                let regex = if let Some(regex) = new.regex.as_ref() {
                                    Some(Regex::new(regex).unwrap())
                                } else {
                                    None
                                };

                                active_rules.push(ActiveRule {
                                    id: new.id.to_string(),
                                    line_offset: 0,
                                    regex
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
                                        search.regex = Some(Regex::new(regex).unwrap())
                                    } else {
                                        search.regex.take();
                                    }
                                }
                            }
                        }
                        FileThreadMsg::Quit => {
                            debug!("Quit signal");
                            break;
                        }
                    }
                }

                if let Some(data) = full_search_data {
                    if let Ok(r) = search(&data, &mut active_rules, 0) {
                        if r.results.len() > 0 {
                            sender(FileViewMsg::Data(0, data, r.results));
                        }
                    }
                } else {
                    if let Ok(data) = log_reader.read().await {
                        let read_bytes = data.len();
                        let mut encoding: Option<&'static dyn encoding::types::Encoding> = None;
                        if let Ok(result) = decode_data(&data, encoding) {
                            if result.encoding.is_some() {
                                encoding = result.encoding;
                            }

                            if let Ok(r) = search(&result.data, &mut active_rules, line_offset) {
                                line_offset += r.lines;
                                if read_bytes > 0 {
                                    sender(FileViewMsg::Data(read_bytes as u64, result.data, r.results));
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

impl Drop for FileView {
    fn drop(&mut self) {
        if let Some(mut child) = self.child_process.take() {
            if let Err(e) = child.kill() {
                eprintln!("Could not kill stern child process: {:?}", e);
            }
            if let Err(e) = child.wait() {
                eprintln!("Could not wait for stern child process: {:?}", e);
            }
        }
    }
}