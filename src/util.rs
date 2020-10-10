use gtk::prelude::*;
use glib::{SignalHandlerId, Value};

use std::io::{BufReader, Read, BufRead};
use std::error::Error;
use encoding::all::{UTF_8};
use encoding::{DecoderTrap, Encoding};
use glib::bitflags::_core::cmp::Ordering;


use std::collections::{HashMap};
use gtk::{TreeViewColumn, CellRendererText, CellRendererToggle, TreeStore};
use std::rc::Rc;
use crate::model::{ActiveRule, SearchResultData, SearchResultMatch};


pub fn enable_auto_scroll(text_view : &sourceview::View) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv, _b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

pub fn decode_data<'a>(buffer: &[u8], _encoding_name: &mut Option<String>) -> Result<String, Box<dyn Error>> {
    let mut data = UTF_8.decode(buffer, DecoderTrap::Ignore)?;
    // let re = Regex::new("\n\r|\r\n|\r")?;
    // data = re.replace_all(&data, "").to_string();
    data = data.replace("\0", "");
    data = data.replace("\n\r", "\n");
    data = data.replace("\r\n", "\n");
    data = data.replace("\r", "\n");
    data = data.replace("", "");
    return Ok(data);
}


pub fn read<T: Read>(stream: &mut T) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut reader = BufReader::new(stream);
    let mut buffer = vec![];
    let mut read_bytes = 0;
    loop {
        let mut tmp = vec![];
        let read = reader.read_until(b'\n', &mut tmp)?;
        if read <= 0 || read_bytes > (1024 * 500) {
            break;
        }

        let last = tmp[tmp.len() -1];
        if last == b'\n' {
            buffer.append(&mut tmp);
            read_bytes += read;
        }

    }

    Ok(buffer)
}

pub fn search(text: &str, active_rules: &mut Vec<ActiveRule>, line_offset: usize) -> Result<SearchResultData, Box<dyn Error>> {
    let lines = text.split("\n").enumerate();
    let mut line_cnt = 0;

    let mut matches: HashMap<String, Vec<SearchResultMatch>> = HashMap::new();
    for (n, line) in lines {
        line_cnt = n;
        for search_data in active_rules.iter_mut() {
            if search_data.line_offset > n {
                continue;
            }

            if !matches.contains_key(&search_data.id) {
                matches.insert(search_data.id.clone(), vec![]);
            }

            if let Some(matches) = matches.get_mut(&search_data.id) {
                if let Some(regex) = &search_data.regex {
                    if text.len() > 0 {
                        for mat in regex.find_iter(&line) {
                            matches.push(SearchResultMatch {
                                line: n + line_offset,
                                start: mat.start(),
                                end: mat.end()
                            });
                        }

                    }
                }
            }

        }
    }


    Ok(SearchResultData {
        lines: line_cnt,
        results: matches
    })
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

pub enum ColumnType {
    String,
    Bool
}

pub fn create_col(title: Option<&str>, idx: i32, col_type: ColumnType, ts: Rc<TreeStore>) -> TreeViewColumn {
    let col = TreeViewColumn::new();
    match col_type {
        ColumnType::String => {
            let cell = CellRendererText::new();
            col.pack_start(&cell, true);
            col.add_attribute(&cell, "text", idx);
            col.set_resizable(true);
            col.set_sort_column_id(idx);
        }
        ColumnType::Bool => {
            let cell = CellRendererToggle::new();
            cell.set_activatable(true);
            cell.connect_toggled(move |e,b| {
                let ts = &*ts;
                if let Some(i) = ts.get_iter(&b) {
                    ts.set_value(&i,1, &Value::from(&!e.get_active()));
                }

            });
            col.pack_start(&cell, true);
            col.add_attribute(&cell, "active", idx);
        }
    }
    if let Some(title) = title {
        col.set_title(title);
    }

    col
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