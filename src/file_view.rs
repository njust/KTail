use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application, ScrolledWindow, TextView, ApplicationWindow, Button, Adjustment, HeaderBar, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation};
use std::time::Duration;
use std::rc::Rc;
use std::error::Error;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use gio::{SimpleAction};

use encoding::all::{UTF_8, WINDOWS_1252, UTF_16BE, UTF_16LE};
use encoding::{Encoding, DecoderTrap};
use std::path::PathBuf;

pub struct FileView {
    path: PathBuf,
    container: gtk::Box
}

impl FileView {
    pub fn new(path: PathBuf) -> Self {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

        let tx = tx.clone();
        let p = path.clone();
        std::thread::spawn(move || {
            let mut offset = 0;
            loop {
                if let Ok((read, s)) = get(&p, offset) {
                    offset += read;
                    tx.send(s);
                }
                std::thread::sleep(Duration::from_millis(900));
            }
        });

        let text_view = TextView::new();
        text_view.connect_size_allocate(|tv,b| {
            if let Some(buffer) = tv.get_buffer() {
                let mut end = buffer.get_end_iter();
                tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
            }
        });

        let sw = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
        sw.set_vexpand(true);
        sw.set_hexpand(true);
        sw.add(&text_view);

        rx.attach(None, move |i| {
            if let Some(buffer) = text_view.get_buffer() {
                if i.len() > 0 {
                    let (_start, mut end) = buffer.get_bounds();
                    buffer.insert(&mut end, &i);
                }
            }
            glib::Continue(true)
        });

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_vexpand(true);
        container.set_hexpand(true);


        container.add(&sw);
        Self {
            path,
            container
        }
    }

    pub fn get_view(&self) -> &gtk::Box {
        &self.container
    }
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
        println!("Dropping file view");
    }
}