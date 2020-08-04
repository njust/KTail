use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ScrolledWindow, TextView, ApplicationWindow, Button, Adjustment, HeaderBar, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation};
use std::time::Duration;
use std::rc::Rc;
use std::error::Error;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use gio::{SimpleAction};
use std::sync::{Arc, Mutex, Condvar};

use encoding::all::{UTF_8, WINDOWS_1252, UTF_16BE, UTF_16LE};
use encoding::{Encoding, DecoderTrap};
use std::path::PathBuf;
use std::thread::JoinHandle;
use glib::SignalHandlerId;

pub struct FileView {
    container: gtk::Box,
    stop_handle: Arc<(Mutex<bool>, Condvar)>,
    text_view: Rc<TextView>,
    autoscroll_handler: Option<SignalHandlerId>,
}

impl FileView {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let tx = tx.clone();

        let stop_handle = Arc::new((Mutex::new(false), Condvar::new()));
        let thread_stop_handle = stop_handle.clone();
        std::thread::spawn(move || {
            let mut offset = 0;
            let (lock, wait_handle) = thread_stop_handle.as_ref();
            let mut stopped = lock.lock().unwrap();

            while !*stopped {
                if let Ok((read, s)) = get(&path, offset) {
                    offset += read;
                    tx.send(s);
                }

                stopped = wait_handle.wait_timeout(stopped, Duration::from_millis(500)).unwrap().0;
            }
            println!("File watcher stopped");
        });

        let text_view = Rc::new(TextView::new());
        let autoscroll_handler = enable_auto_scroll(&*text_view);

        let scroll_wnd = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
        scroll_wnd.set_vexpand(true);
        scroll_wnd.set_hexpand(true);
        scroll_wnd.add(&*text_view);

        {
            let text_view = text_view.clone();
            rx.attach(None, move |i| {
                if let Some(buffer) = &text_view.get_buffer() {
                    if i.len() > 0 {
                        let (_start, mut end) = buffer.get_bounds();
                        buffer.insert(&mut end, &i);
                    }
                }
                glib::Continue(true)
            });
        }

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);

        container.add(&scroll_wnd);
        Self {
            container,
            stop_handle,
            text_view,
            autoscroll_handler: Some(autoscroll_handler)
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

    pub fn get_view(&self) -> &gtk::Box {
        &self.container
    }
}

pub fn enable_auto_scroll(text_view : &TextView) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv,b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

fn get(path: &PathBuf, start: u64) -> Result<(u64, String), Box<dyn Error>> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    if start > 0 {
        reader.seek(SeekFrom::Start(start));
    }

    let read = reader.read_to_end(&mut buffer)?;
    let s = UTF_16LE.decode(buffer.as_slice(), DecoderTrap::Replace)?;
    Ok((read as u64, s))
}

impl Drop for FileView {
    fn drop(&mut self) {
        let &(ref lock, ref cvar) = self.stop_handle.as_ref();
        let mut stop = lock.lock().unwrap();
        *stop = true;
        cvar.notify_one();
    }
}