use gtk::prelude::*;
use gtk::{TextView, TreeViewColumn, CellRendererText, TreePath, CellRenderer, ColorChooserDialog, ApplicationWindow, ResponseType};
use glib::{SignalHandlerId, Sender};
use std::path::PathBuf;
use std::io::{BufReader, SeekFrom, Read, Seek};
use std::error::Error;
use encoding::all::UTF_16LE;
use encoding::{Encoding, DecoderTrap};
use glib::bitflags::_core::cmp::Ordering;
use regex::Regex;
use crate::file_view::workbench::{Msg, RuleMsg};

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

pub fn create_col(title: &str, idx: i32, tx: Sender<Msg>, color: bool) -> TreeViewColumn {
    let column = TreeViewColumn::new();
    let renderer = CellRendererText::new();
    column.pack_start(&renderer, true);
    renderer.connect_edited(move |e, f, g| {
        tx.send(Msg::RuleMsg(RuleMsg::RuleChanged(f, idx as u32, String::from(g))));
    });

    if color {
        column.add_attribute(&renderer, "cell-background", idx);
        column.add_attribute(&renderer, "text", idx);
    }else {
        column.add_attribute(&renderer, "text", idx);
        renderer.set_property_editable(true);
    }

    column.set_title(title);
    column.set_resizable(true);
    column.set_sort_column_id(idx);
    column.set_expand(true);
    column
}


pub struct SortedListCompare<'a, 'b, T: PartialOrd> {
    lh: &'a Vec<T>,
    rh: &'b Vec<T>,
    lhi: usize,
    rhi: usize,
}

#[derive(Debug)]
pub enum CompareResult<'a, 'b, T:PartialOrd> {
    Equal(&'a T, &'b T),
    MissesLeft(&'b T),
    MissesRight(&'a T)
}

impl<'a, 'b, T: PartialOrd> Iterator for SortedListCompare<'a, 'b, T> {
    type Item = CompareResult<'a, 'b, T>;
    fn next(&mut self) -> Option<Self::Item> {
        let lh = self.lh.get(self.lhi);
        let rh = self.rh.get(self.rhi);
        if lh.is_none() && rh.is_none() {
            return None;
        }

        if lh.is_none() {
            self.rhi +=1;
            return Some(CompareResult::MissesLeft(rh.unwrap()));
        }

        if rh.is_none() {
            self.lhi +=1;
            return Some(CompareResult::MissesRight(lh.unwrap()));
        }

        let lh = lh.unwrap();
        let rh = rh.unwrap();
        match lh.partial_cmp(rh) {
            Some(c) => {
                match c {
                    Ordering::Less => {
                        self.lhi += 1;
                        Some(CompareResult::MissesRight(lh))
                    }
                    Ordering::Greater => {
                        self.rhi += 1;
                        Some(CompareResult::MissesLeft(rh))
                    }
                    Ordering::Equal => {
                        self.rhi += 1;
                        self.lhi += 1;
                        Some(CompareResult::Equal(lh, rh))
                    }
                }
            }
            None => {
                Some(CompareResult::MissesRight(lh))
            }
        }
    }
}

impl<'a, 'b, T:PartialOrd> SortedListCompare<'a, 'b, T> {
    pub fn new(lh: &'a mut Vec<T>, rh: &'b mut Vec<T>) -> Self {
        SortedListCompare {
            lh,
            rh,
            lhi: 0,
            rhi: 0
        }
    }
}