use gtk::prelude::*;

use gtk::{ScrolledWindow, TextView, Orientation, TextBuffer, TextTag, TextTagTable};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Condvar};
use std::path::PathBuf;
use glib::{SignalHandlerId, Receiver, Sender};
use crate::util::{enable_auto_scroll, read_file, search, SortedListCompare, CompareResult};
use crate::rules::{Rule};
use crate::{SEARCH_TAG};
use crate::file::{FileThreadMsg, CustomRule, FileUiMsg, RuleChanges, ActiveRule, SearchResult};

pub struct FileView {
    container: gtk::Box,
    stop_handle: Arc<(Mutex<bool>, Condvar)>,
    text_view: Rc<TextView>,
    thread_action_sender: std::sync::mpsc::Sender<FileThreadMsg>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<CustomRule>,
}

impl FileView {
    pub fn new(path: PathBuf) -> Self {
        let (ui_action_sender, ui_action_receiver) =
            glib::MainContext::channel::<FileUiMsg>(glib::PRIORITY_DEFAULT);

        let (thread_action_sender, thread_action_receiver) =
            std::sync::mpsc::channel::<FileThreadMsg>();

        let stop_handle = Arc::new((Mutex::new(false), Condvar::new()));
        register_file_watcher_thread(path, ui_action_sender.clone(), stop_handle.clone(), thread_action_receiver);

        let search = TextTag::new(Some(SEARCH_TAG));
        search.set_property_background(Some("#FFF135"));

        let tag_table = TextTagTable::new();
        tag_table.add(&search);

        let text_buffer = TextBuffer::new(Some(&tag_table));
        let text_view = Rc::new(TextView::with_buffer(&text_buffer));

        let scroll_wnd = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_hexpand(true);
        scroll_wnd.add(&*text_view);

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);
        container.add(&scroll_wnd);

        attach_text_view_update(text_view.clone(), ui_action_receiver);

        Self {
            container,
            stop_handle,
            text_view,
            thread_action_sender,
            autoscroll_handler: None,
            rules: vec![],
        }
    }

    pub fn search(&mut self, search_text: String) {
        self.thread_action_sender.send(FileThreadMsg::AddRule(Rule::UserSearch(search_text))).expect("Could not send add rule");
    }

    pub fn clear_search(&mut self, search: &str) {
        self.thread_action_sender.send(FileThreadMsg::DeleteRule(Rule::UserSearch(search.to_string()))).expect("Could not send delete rule");
        clear_search(&self.text_view);
    }

    pub fn apply_rules(&mut self, mut rules: Vec<CustomRule>) {
        let mut add = vec![];
        let mut remove = vec![];
        let mut update = vec![];

        let compare_results = SortedListCompare::new(&mut self.rules, &mut rules);
        for compare_result in compare_results {
            let text_view = self.text_view.clone();
            match compare_result {
                CompareResult::MissesLeft(new) => {
                    add.push(new.clone());
                    if let Some(tags) = text_view.get_buffer()
                        .and_then(|buffer| buffer.get_tag_table()) {
                        let tag = TextTag::new(Some(&new.id.to_string()));
                        tag.set_property_background(new.color.as_ref().map(|c|c.as_str()));
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
                        tag.set_property_background(right.color.as_ref().map(|s|s.as_str()));
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
        self.rules = rules;
        self.thread_action_sender.send(FileThreadMsg::ApplyRules(RuleChanges {
            add,
            remove,
            update
        })).expect("Could not send apply rules");
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

fn clear_search(text_view: &TextView) {
    if let Some(buffer) = text_view.get_buffer() {
        let (start, end) = buffer.get_bounds();
        buffer.remove_tag_by_name(SEARCH_TAG, &start, &end);
    }
}

fn attach_text_view_update(text_view: Rc<TextView>, rx: Receiver<FileUiMsg>) {
    let text_view = text_view.clone();
    rx.attach(None, move |msg| {
        match msg {
            FileUiMsg::Data(read, data, search_result_list) => {
                if let Some(buffer) = &text_view.get_buffer() {
                    let (_start, mut end) = buffer.get_bounds();
                    let line_offset = end.get_line();
                    if read > 0 {
                        buffer.insert(&mut end, &data);
                    }
                    for search_result in search_result_list {
                        for search_match in search_result.matches {
                            let line = if search_result.with_offset {
                                line_offset + search_match.line as i32
                            } else {
                                search_match.line as i32
                            };
                            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
                            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
                            buffer.apply_tag_by_name(&search_result.tag, &iter_start, &iter_end);
                        }
                    }
                }
            }
            FileUiMsg::Clear => {
                if let Some(buffer) = &text_view.get_buffer() {
                    buffer.set_text("");
                }
            }
        }
        glib::Continue(true)
    });
}



fn register_file_watcher_thread(path: PathBuf, tx: Sender<FileUiMsg>, thread_stop_handle: Arc<(Mutex<bool>, Condvar)>, rx: std::sync::mpsc::Receiver<FileThreadMsg>) {
    std::thread::spawn(move || {
        let mut file_byte_offset = 0;
        let mut utf8_byte_offset = 0;
        let (lock, wait_handle) = thread_stop_handle.as_ref();
        let mut stopped = lock.lock().unwrap();

        let mut active_rules: Vec<ActiveRule> = vec![];
        while !*stopped {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let len = metadata.len();
                if len < file_byte_offset {
                    file_byte_offset = 0;
                    utf8_byte_offset = 0;
                    tx.send(FileUiMsg::Clear).expect("Could not send clear file");
                }
            }

            let mut read_full_file = false;
            if let Some(msg) = rx.try_iter().peekable().peek() {
                match msg {
                    FileThreadMsg::AddRule(rule) => {
                        read_full_file = true;
                        active_rules.push(ActiveRule {
                            rule: rule.clone(),
                            is_new: true,
                        });
                    }
                    FileThreadMsg::DeleteRule(rule) => {
                        if let Some((idx, _search)) = active_rules.iter().enumerate().find(|(_, item)| &item.rule == rule) {
                            active_rules.remove(idx);
                        }
                    }
                    FileThreadMsg::ApplyRules(changes) => {
                        for new in &changes.add {
                            if new.regex.is_some() {
                                read_full_file = true;
                            }
                            active_rules.push(ActiveRule {
                                rule: Rule::CustomRule(new.clone()),
                                is_new: true,
                            });
                        }
                        for remove in &changes.remove {
                            if let Some((idx, _item)) = active_rules.iter().enumerate().find(|(_, e)| &e.rule.get_id() == remove) {
                                active_rules.remove(idx);
                            }
                        }

                        for update in &changes.update {
                            if let Some((idx, _search)) = active_rules.iter().enumerate().find(|(_, item)| item.rule.get_id() == update.id.to_string()) {
                                if update.regex.is_some() {
                                    read_full_file = true;
                                }

                                active_rules.remove(idx);
                                active_rules.push(ActiveRule {
                                    rule: Rule::CustomRule(update.clone()),
                                    is_new: true,
                                });
                            }
                        }
                    }
                }
            }

            let tmp_file_offset = if read_full_file { 0 } else { file_byte_offset };
            if let Ok((read_bytes, content)) = read_file(&path, tmp_file_offset) {
                let read_utf8 = content.as_bytes().len();
                let mut re_list_matches = vec![];
                for search_data in active_rules.iter_mut() {
                    let search_content = if search_data.is_new {
                        &content[0..]
                    } else {
                        if read_utf8 > utf8_byte_offset {
                            &content[utf8_byte_offset..]
                        } else {
                            &content[0..]
                        }
                    };

                    if let Some(regex) = search_data.rule.get_regex() {
                        let matches = search(search_content, regex).unwrap_or(vec![]);
                        if matches.len() > 0 {
                            re_list_matches.push(SearchResult {
                                matches,
                                tag: search_data.rule.get_tag(),
                                with_offset: !search_data.is_new,
                            });
                        }
                        search_data.is_new = false;
                    }
                }

                let delta_content = if read_full_file {
                    let res = if read_bytes >= file_byte_offset {
                        &content[utf8_byte_offset as usize..]
                    } else {
                        &content[0..]
                    };

                    utf8_byte_offset = read_utf8;
                    file_byte_offset = read_bytes;
                    res
                } else {
                    utf8_byte_offset += read_utf8;
                    file_byte_offset += read_bytes;
                    &content[0..]
                };

                tx.send(FileUiMsg::Data(read_bytes, String::from(delta_content), re_list_matches)).expect("Could not send file data");
            }

            stopped = wait_handle.wait_timeout(stopped, Duration::from_millis(500)).unwrap().0;
        }
        println!("File watcher stopped");
    });
}

impl Drop for FileView {
    fn drop(&mut self) {
        let &(ref lock, ref cvar) = self.stop_handle.as_ref();
        let mut stop = lock.lock().unwrap();
        *stop = true;
        cvar.notify_one();
    }
}