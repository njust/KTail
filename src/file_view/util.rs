use gtk::prelude::*;
use gtk::TextView;
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

pub fn search(text: &String, search: String) -> Result<Vec<(usize, usize)>, Box<dyn Error>> {
    let re = Regex::new(&search)?;
    let mut temp_matches = vec![];
    let mut map = std::collections::HashMap::new();
    for mat in re.find_iter(&text) {
        map.insert(mat.start(), 0);
        map.insert(mat.end(), 0);
        temp_matches.push((mat.start(), mat.end()));
    }

    let mut current = 0;
    for (index, current_char) in text.chars().into_iter().enumerate() {
        current += current_char.len_utf8();
        if let Some(_) = map.get(&current) {
            map.insert(current, index + 1);
        }
    }

    let mut matches = vec![];
    for (start, end) in temp_matches {
        if let (Some(mapped_start), Some(mapped_end)) = (map.get(&start), map.get(&end)) {
            matches.push((*mapped_start, *mapped_end));
        }
    }

    matches.sort_by(|a, b| {
        if a.0 > b.0 {
            return Ordering::Greater;
        }else {
            return Ordering::Less;
        }
    });
    Ok(matches)
}