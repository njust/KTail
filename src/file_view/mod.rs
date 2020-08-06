use gtk::prelude::*;

use gtk::{ScrolledWindow, TextView, Orientation, TextBuffer, TextTag, TextTagTable};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Condvar};
use std::path::PathBuf;
use glib::{SignalHandlerId, Receiver, Sender};
use crate::file_view::util::{enable_auto_scroll, read_file, search};

pub mod workbench;
pub mod toolbar;
pub mod util;


const ERROR_FATAL: &'static str = "ERROR_FATAL";

pub struct FileView {
    container: gtk::Box,
    stop_handle: Arc<(Mutex<bool>, Condvar)>,
    text_view: Rc<TextView>,
    autoscroll_handler: Option<SignalHandlerId>,
}

enum FileMsg {
    Data(u64, String, Vec<(usize, usize)>),
    Clear
}

impl FileView {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let stop_handle = Arc::new((Mutex::new(false), Condvar::new()));
        register_file_watcher(path, tx, stop_handle.clone());

        let error_fatal = TextTag::new(Some(ERROR_FATAL));
        error_fatal.set_property_foreground(Some("orange"));

        let tag_table = TextTagTable::new();
        tag_table.add(&error_fatal);

        let text_buffer = TextBuffer::new(Some(&tag_table));
        let text_view = Rc::new(TextView::with_buffer(&text_buffer));
        let autoscroll_handler= enable_auto_scroll(&*text_view);

        let scroll_wnd = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_hexpand(true);
        scroll_wnd.add(&*text_view);

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);
        container.add(&scroll_wnd);

        attach_text_view_update(text_view.clone(), rx);

        Self {
            container,
            stop_handle,
            text_view,
            autoscroll_handler: Some(autoscroll_handler)
        }
    }

    fn search(&mut self, text: String) {
        println!("Search: {}", text);
    }

    fn toggle_autoscroll(&mut self, enable: bool) {
        if enable {
            self.enable_auto_scroll();
        }else {
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

fn attach_text_view_update(text_view: Rc<TextView>, rx: Receiver<FileMsg>) {
    let text_view = text_view.clone();
    rx.attach(None, move |msg| {
        match msg {
            FileMsg::Data(read, data, matches) => {
                if let Some(buffer) = &text_view.get_buffer() {
                    let (_start, mut end) = buffer.get_bounds();
                    let char_offset = end.get_offset();
                    if read > 0 {
                        buffer.insert(&mut end, &data);
                    }

                    for (start, end) in matches {
                        let iter_start = buffer.get_iter_at_offset(char_offset + start as i32);
                        let iter_end = buffer.get_iter_at_offset(char_offset + end as i32);
                        buffer.apply_tag_by_name(ERROR_FATAL, &iter_start, &iter_end);
                    }
                }
            }
            FileMsg::Clear => {
                if let Some(buffer) = &text_view.get_buffer() {
                    buffer.set_text("");
                }
            }
        }
        glib::Continue(true)
    });
}

fn register_file_watcher(path: PathBuf, tx: Sender<FileMsg>, thread_stop_handle: Arc<(Mutex<bool>, Condvar)>) {
    std::thread::spawn(move || {
        let mut offset = 0;
        let (lock, wait_handle) = thread_stop_handle.as_ref();
        let mut stopped = lock.lock().unwrap();

        while !*stopped {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let len = metadata.len();
                if len < offset {
                    offset = 0;
                    tx.send(FileMsg::Clear);
                }
            }

            if let Ok((read, s)) = read_file(&path, offset) {
                // Todo: Use option of vec
                let matches = search(&s, String::from(r".*\s((?i)error|fatal(?-i))\s.*")).unwrap_or(vec![]);
                offset += read;
                tx.send(FileMsg::Data(read, s, matches));
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