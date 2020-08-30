use gtk::{prelude::*, WrapMode};

use gtk::{ScrolledWindow, Orientation, TextTag, TextTagTable};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Condvar};
use std::path::PathBuf;
use glib::{SignalHandlerId};
use crate::util::{enable_auto_scroll, read_file, search, SortedListCompare, CompareResult, CREATE_NO_WINDOW};
use crate::{FileViewMsg, SearchResult, FileViewData};
use crate::file::{FileThreadMsg, Rule, RuleChanges, ActiveRule};
use sourceview::{ViewExt};
use subprocess::{PopenConfig, Redirection, Popen};
use uuid::Uuid;

pub struct FileView {
    container: gtk::Box,
    text_view: Rc<sourceview::View>,
    autoscroll_handler: Option<SignalHandlerId>,
    rules: Vec<Rule>,
    kube_log_process: Option<Popen>,
    kube_log_path: Option<PathBuf>,
    stop_handle: Option<Arc<(Mutex<bool>, Condvar)>>,
    thread_action_sender: Option<std::sync::mpsc::Sender<FileThreadMsg>>,
}

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

        Self {
            container,
            text_view,
            autoscroll_handler: None,
            rules: vec![],
            kube_log_process: None,
            kube_log_path: None,
            thread_action_sender: None,
            stop_handle: None,
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
                    for search_result in search_result_list {
                        for search_match in search_result.matches {
                            let line = search_match.line as i32;
                            let iter_start = buffer.get_iter_at_line_index(line, search_match.start as i32);
                            let iter_end = buffer.get_iter_at_line_index(line, search_match.end as i32);
                            buffer.apply_tag_by_name(&search_result.tag, &iter_start, &iter_end);
                        }
                    }
                }
            }
            FileViewMsg::Clear => {
                if let Some(buffer) = &self.text_view.get_buffer() {
                    buffer.set_text("");
                }
            }
        }
    }

    pub fn apply_rules(&mut self, mut rules: Vec<Rule>) {
        let mut add = vec![];
        let mut remove = vec![];
        let mut update = vec![];

        rules.sort_by_key(|i| i.id);
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
        if let Some(thread_action_sender) = self.thread_action_sender.as_ref() {
            thread_action_sender.send(FileThreadMsg::ApplyRules(RuleChanges {
                add,
                remove,
                update
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
        let mut utf8_byte_offset = 0;
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
                    utf8_byte_offset = 0;
                    sender(FileViewMsg::Clear);
                }
            }

            let mut read_full_file = false;
            if let Some(msg) = rx.try_iter().peekable().peek() {
                match msg {
                    FileThreadMsg::ApplyRules(changes) => {
                        for new in &changes.add {
                            if new.regex.is_some() {
                                read_full_file = true;
                            }
                            active_rules.push(ActiveRule {
                                rule: new.clone(),
                                line_offset: 0,
                            });
                        }
                        for remove in &changes.remove {
                            if let Some((idx, _item)) = active_rules.iter().enumerate().find(|(_, e)| &e.rule.id.to_string() == remove) {
                                active_rules.remove(idx);
                            }
                        }

                        for update in &changes.update {
                            if let Some((_idx, search)) = active_rules.iter_mut().enumerate().find(|(_, item)| item.rule.id == update.id) {
                                if update.regex.is_some() {
                                    read_full_file = true;
                                    search.rule.regex = update.regex.clone();
                                    search.line_offset = 0;
                                }else {
                                    search.rule.regex.take();
                                }
                            }
                        }
                    }
                }
            }

            let tmp_file_offset = if read_full_file { 0 } else { file_byte_offset };
            if let Ok(result) = read_file(&path, tmp_file_offset, encoding) {
                if result.encoding.is_some() {
                    encoding = result.encoding;
                }
                let content = result.data;
                let read_bytes = result.read_bytes;
                let read_utf8 = content.as_bytes().len();

                let mut re_list_matches = vec![];
                for search_data in active_rules.iter_mut() {
                    let search_content= content.as_str();

                    if let Some(regex) = &search_data.rule.regex {
                        if regex.len() > 0 && search_content.len() > 0 {
                            if let Ok(search_result) = search(search_content, regex, search_data.line_offset, read_full_file) {
                                search_data.line_offset += search_result.lines;

                                if search_result.matches.len() > 0 {
                                    re_list_matches.push(SearchResult {
                                        matches: search_result.matches,
                                        tag: search_data.rule.id.to_string(),
                                    });
                                }

                            }
                        }

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

                sender(FileViewMsg::Data(read_bytes, String::from(delta_content), re_list_matches));
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