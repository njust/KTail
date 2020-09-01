use gtk::{prelude::*, TextView};

use gtk::{ScrolledWindow, Orientation, TextTag, TextTagTable};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Condvar};
use std::path::PathBuf;
use glib::{SignalHandlerId};
use crate::util::{enable_auto_scroll, read_file, SortedListCompare, CompareResult, CREATE_NO_WINDOW, search, decode_data};
use crate::{FileViewMsg, FileViewData, SearchResultMatch};
use crate::file::{FileThreadMsg, Rule, RuleChanges, ActiveRule};
use sourceview::{ViewExt};
use subprocess::{PopenConfig, Redirection, Popen};
use uuid::Uuid;
use regex::Regex;
use std::collections::HashMap;

pub struct FileView {
    container: gtk::Box,
    text_view: Rc<sourceview::View>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<Rule>,
    kube_log_process: Option<Popen>,
    kube_log_path: Option<PathBuf>,
    stop_handle: Option<Arc<(Mutex<bool>, Condvar)>>,
    thread_action_sender: Option<std::sync::mpsc::Sender<FileThreadMsg>>,
    result_map: HashMap<String, Vec<SearchResultMatch>>,
    result_cursor: HashMap<String, usize>,
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

        let stop_handle = Arc::new((Mutex::new(false), Condvar::new()));

        let file_thread_tx = sender.clone();
        match data {
            FileViewData::File(path) => {
                let path = path.clone();
                register_file_watcher_thread(move |msg| {
                    file_thread_tx(msg);
                }, &path,  stop_handle.clone(), thread_action_receiver);
            }
            FileViewData::Kube(services) => {
                let tmp_file = std::env::temp_dir().join(Uuid::new_v4().to_string());
                println!("{:?}", tmp_file);
                if let Ok(file) = std::fs::OpenOptions::new()
                    .write(true)
                    .append(true)
                    .create(true)
                    .open(&tmp_file) {

                    let mut cfg = PopenConfig {
                        stdout: Redirection::File(file),
                        detached: true,
                        ..Default::default()
                    };

                    #[cfg(target_family = "windows")]
                        {
                            cfg.creation_flags = CREATE_NO_WINDOW;
                        }

                    let template = if services.len() == 1 {
                        "{{.Message}}"
                    }else {
                        "{{.ContainerName}} {{.Message}}"
                    };

                    let process = subprocess::Popen::create(&["stern", "--since", "12h", "--template", template, &services.join("|")], cfg).unwrap();

                    self.kube_log_process = Some(process);
                }

                register_file_watcher_thread(move |msg| {
                    file_thread_tx(msg);
                }, &tmp_file, stop_handle.clone(), thread_action_receiver);
                self.kube_log_path = Some(tmp_file);
            }
        }

        self.stop_handle = Some(stop_handle);
    }
    pub fn new() -> Self {
        let tag_table = TextTagTable::new();
        let current_cursor_tag = gtk::TextTag::new(Some(CURRENT_CURSOR_TAG));
        current_cursor_tag.set_property_background(Some("red"));
        tag_table.add(&current_cursor_tag);

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
        let result_cursor: HashMap<String, usize> = HashMap::new();

        Self {
            container,
            text_view,
            autoscroll_handler: None,
            rules: vec![],
            kube_log_process: None,
            kube_log_path: None,
            thread_action_sender: None,
            stop_handle: None,
            result_map,
            result_cursor
        }
    }

    fn buffer_set_cursor(text_view: &sourceview::View, search_match: &SearchResultMatch) {
        if let Some(buffer) = text_view.get_buffer() {
            let (start, end) = buffer.get_bounds();
            buffer.remove_tag_by_name(CURRENT_CURSOR_TAG, &start, &end);
            let line = search_match.line as i32;
            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
            buffer.apply_tag_by_name(CURRENT_CURSOR_TAG, &iter_start, &iter_end);
        }
    }

    pub fn select_prev(&mut self, id: &str) {
        if let Some(current) = self.result_cursor.get_mut(id) {
            let prev_pos = if *current > 0 { *current -1 }else {0};
            if let Some(d) = self.result_map.get(id) {
                if let Some(next) = d.get(prev_pos) {
                    Self::buffer_set_cursor(&*self.text_view, &next);
                }
            }
            *current = prev_pos;
        } else {
            if let Some(d) = self.result_map.get(id) {
                if let Some(first) = d.get(d.len() - 1) {
                    Self::buffer_set_cursor(&*self.text_view, &first);
                    self.result_cursor.insert(id.to_string(), d.len() - 1);
                }
            }
        }
    }

    pub fn select_next(&mut self, id: &str) {
        if let Some(current) = self.result_cursor.get_mut(id) {
            let mut next_pos = 0;
            if let Some(d) = self.result_map.get(id) {
                next_pos = if *current < d.len() -1 { *current +1 }else {0};
                if let Some(next) = d.get(next_pos) {
                    Self::buffer_set_cursor(&*self.text_view, &next);
                }
            }
            *current = next_pos;
        }else {
            if let Some(d) = self.result_map.get(id) {
                if let Some(first) = d.get(0) {
                    Self::buffer_set_cursor(&*self.text_view, &first);
                    self.result_cursor.insert(id.to_string(), 0);
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
            FileViewMsg::Clear => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    buffer.set_text("");
                    self.result_cursor.clear();
                    self.result_map.clear();
                }
            }
        }
    }

    pub fn apply_rules(&mut self, mut rules: Vec<Rule>) {
        let mut add = vec![];
        let mut remove = vec![];
        let mut update = vec![];

        rules.sort_by_key(|i| i.id);
        let init = self.rules.len() <= 0;
        let compare_results = SortedListCompare::new(&mut self.rules, &mut rules);
        for compare_result in compare_results {
            let text_view = self.text_view.clone();
            match compare_result {
                CompareResult::MissesLeft(new) => {
                    add.push(new.clone());
                    if let Some(tags) = text_view.get_buffer()
                        .and_then(|buffer| buffer.get_tag_table()) {
                        let tag = TextTag::new(Some(&new.id.to_string()));
                        tag.set_property_foreground(new.color.as_ref().map(|c|c.as_str()));
                        tags.add(&tag);
                    }
                }
                CompareResult::MissesRight(delete) => {
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
                        tag.set_property_foreground(right.color.as_ref().map(|s|s.as_str()));
                    }
                    if left.regex != right.regex {
                        update.push(right.clone());
                        if let Some(tb) = text_view.get_buffer() {
                            let (start, end) = tb.get_bounds();
                            tb.remove_tag_by_name(&left.id.to_string(), &start, &end);
                        }
                    }
                }
            }
        }
        let mut data :Option<String> = None;
        if !init {
            let text_view = self.text_view.clone();
            if let Some(tb) = text_view.get_buffer() {
                let (start, end) = tb.get_bounds();
                data = tb.get_text(&start, &end, false).map(|s|s.to_string());
            }
        }

        self.rules = rules;
        if let Some(thread_action_sender) = self.thread_action_sender.as_ref() {
            thread_action_sender.send(FileThreadMsg::ApplyRules(RuleChanges {
                add,
                remove,
                update,
                data,
            })).expect("Could not send apply rules");
        }
    }

    pub fn toggle_autoscroll(&mut self, enable: bool) {
        if enable {
            self.enable_auto_scroll();
        } else {
            self.disable_auto_scroll();
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

fn register_file_watcher_thread<T>(sender: T, path: &PathBuf, thread_stop_handle: Arc<(Mutex<bool>, Condvar)>, rx: std::sync::mpsc::Receiver<FileThreadMsg>)
    where T : 'static + Send + Clone + Fn(FileViewMsg)
{
    let path = path.clone();
    std::thread::spawn(move || {
        let mut file_byte_offset = 0;
        let mut line_offset = 0;
        let (lock, wait_handle) = thread_stop_handle.as_ref();
        let mut stopped = lock.lock().unwrap();

        let mut active_rules = vec![];
        let mut encoding: Option<&'static dyn encoding::types::Encoding> = None;
        while !*stopped {
            if !path.exists() {
                continue;
            }

            if let Ok(metadata) = std::fs::metadata(&path) {
                let len = metadata.len();
                if len <= 0 {
                    continue;
                }

                if len < file_byte_offset {
                    file_byte_offset = 0;
                    line_offset = 0;
                    sender(FileViewMsg::Clear);
                }
            }

            let mut full_search_data = None;
            if let Some(msg) = rx.try_iter().peekable().peek() {
                match msg {
                    FileThreadMsg::ApplyRules(changes) => {
                        full_search_data = changes.data.clone();
                        for new in &changes.add {
                            let regex = if let Some(regex) = new.regex.as_ref() {

                                Some(Regex::new(regex).unwrap())
                            }else {
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
                                }else {
                                    search.regex.take();
                                }
                            }
                        }
                    }
                }
            }

            if let Some(data) = full_search_data {
                if let Ok(r) = search(&data, &mut active_rules, 0) {
                    if r.results.len() > 0 {
                        println!("Results: {}", r.results.len());
                        sender(FileViewMsg::Data(0, data, r.results));
                    }
                }
            } else {
                if let Ok(data) = read_file(&path,  file_byte_offset) {
                    if let Ok(result) = decode_data(&data, encoding) {
                        file_byte_offset += result.read_bytes;
                        if result.encoding.is_some() {
                            encoding = result.encoding;
                        }

                        if let Ok(r) = search(&result.data, &mut active_rules, line_offset) {
                            line_offset += r.lines;
                            if result.read_bytes > 0 {
                                println!("Results: {}", r.results.len());
                                sender(FileViewMsg::Data(result.read_bytes, result.data, r.results));
                            }
                        }
                    }
                }
            }

            stopped = wait_handle.wait_timeout(stopped, Duration::from_millis(500)).unwrap().0;
        }
        println!("File watcher stopped");
    });
}

impl Drop for FileView {
    fn drop(&mut self) {
        if let Some(stop_handle) = self.stop_handle.take() {
            let &(ref lock, ref cvar) = stop_handle.as_ref();
            let mut stop = lock.lock().unwrap();
            *stop = true;
            cvar.notify_one();
        }

        if let Some(mut p) = self.kube_log_process.take() {
            println!("Waiting process to exit");
            match p.kill() {
                Ok(_) => println!("Killed subprocess"),
                Err(e) => eprintln!("Failed to kill subprocess: {}", e),
            }
            println!("OK!");
        }
        if let Some(tmp_file) = self.kube_log_path.take() {
            if let Err(e) = std::fs::remove_file(&tmp_file) {
                eprintln!("Could not delete tmp file: {:?} error was: {}", tmp_file, e);
            }
        }
    }
}