use gtk::prelude::*;
use gtk::{TextView, TreeViewColumn, CellRendererText, TreePath};
use glib::SignalHandlerId;
use std::path::PathBuf;
use std::io::{BufReader, SeekFrom, Read, Seek};
use std::error::Error;
use encoding::all::UTF_16LE;
use encoding::{Encoding, DecoderTrap};
use glib::bitflags::_core::cmp::Ordering;
use regex::Regex;

pub fn enable_auto_scroll(text_view : &TextView) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv, _b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

pub fn read_file(path: &PathBuf, start: u64) -> Result<(u64, String), Box<dyn Error>> {
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

pub fn search(text: &str, search: &str) -> Result<Vec<(usize, usize, usize)>, Box<dyn Error>> {
    let lines = text.split("\n");
    let re = Regex::new(search)?;
    let mut matches = vec![];
    for (n, line) in lines.enumerate() {
        for mat in re.find_iter(&line) {
            matches.push((n, mat.start(), mat.end()));
        }
    }
    Ok(matches)
}

pub fn create_col<T: Fn(&CellRendererText, TreePath, &str) + 'static>(title: &str, idx: i32, t: T) -> TreeViewColumn {
    let col = TreeViewColumn::new();
    let cell = CellRendererText::new();
    cell.connect_edited(move |e,f,g| {
       t(e,f,g);
    });
    cell.set_property_editable(true);
    col.pack_start(&cell, true);
    col.add_attribute(&cell, "text", idx);
    col.set_title(title);
    col.set_resizable(true);
    col.set_sort_column_id(idx);
    col.set_expand(true);
    col
}