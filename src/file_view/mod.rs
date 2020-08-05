use gtk::prelude::*;

use gtk::{ScrolledWindow, TextView, Orientation};
use std::time::Duration;
use std::rc::Rc;
use std::sync::{Arc, Mutex, Condvar};
use encoding::{Encoding, DecoderTrap};
use std::path::PathBuf;
use glib::{SignalHandlerId, Receiver, Sender};
use crate::file_view::util::{enable_auto_scroll, read_file};

pub mod workbench;
pub mod toolbar;
pub mod util;

pub struct FileView {
    container: gtk::Box,
    stop_handle: Arc<(Mutex<bool>, Condvar)>,
    text_view: Rc<TextView>,
    autoscroll_handler: Option<SignalHandlerId>,
}

enum Msg {
    Data(u64, String),
    Clear
}

impl FileView {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let stop_handle = Arc::new((Mutex::new(false), Condvar::new()));
        register_file_watcher(path, tx, stop_handle.clone());

        let text_view = Rc::new(TextView::new());
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

    fn toggle_autoscroll(&mut self) {
        if self.is_auto_scroll_enabled() {
            self.disable_auto_scroll();
        }else {
            self.enable_auto_scroll();
        }
    }

    pub fn enable_auto_scroll(&mut self) {
        let handler = enable_auto_scroll(&*self.text_view);
        self.autoscroll_handler = Some(handler);
    }

    pub fn is_auto_scroll_enabled(&self) -> bool {
        self.autoscroll_handler.is_some()
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

fn attach_text_view_update(text_view: Rc<TextView>, rx: Receiver<Msg>) {
    let text_view = text_view.clone();
    rx.attach(None, move |msg| {
        match msg {
            Msg::Data(read, data) => {
                if let Some(buffer) = &text_view.get_buffer() {
                    if read > 0 {
                        let (_start, mut end) = buffer.get_bounds();
                        buffer.insert(&mut end, &data);
                    }
                }
            }
            Msg::Clear => {
                if let Some(buffer) = &text_view.get_buffer() {
                    buffer.set_text("");
                }
            }
        }
        glib::Continue(true)
    });
}

fn register_file_watcher(path: PathBuf, tx: Sender<Msg>, thread_stop_handle: Arc<(Mutex<bool>, Condvar)>) {
    std::thread::spawn(move || {
        let mut offset = 0;
        let (lock, wait_handle) = thread_stop_handle.as_ref();
        let mut stopped = lock.lock().unwrap();

        while !*stopped {
            if let Ok(metadata) = std::fs::metadata(&path) {
                let len = metadata.len();
                if len < offset {
                    offset = 0;
                    tx.send(Msg::Clear);
                }
            }

            if let Ok((read, s)) = read_file(&path, offset) {
                offset += read;
                tx.send(Msg::Data(read, s));
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