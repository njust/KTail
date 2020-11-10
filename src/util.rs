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
use crate::model::{ActiveRule, SearchResultData, SearchResultMatch, LogReplacer};

pub fn enable_auto_scroll(text_view : &sourceview::View) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv, _b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

pub fn decode_data<'a>(buffer: &[u8], _encoding_name: &mut Option<String>, _replacers: &Vec<LogReplacer>) -> Result<String, Box<dyn Error>> {
    let mut data = UTF_8.decode(buffer, DecoderTrap::Ignore)?;
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

pub fn search(text: String, active_rules: &mut Vec<ActiveRule>, full_search: bool) -> Result<SearchResultData, Box<dyn Error>> {
    let lines = text.split("\n").enumerate();
    let mut matches: HashMap<String, Vec<SearchResultMatch>> = HashMap::new();

    let mut data = String::new();
    let mut line_count = 0;

    for (n, line) in lines {
        if line.len() <= 0 {
            continue;
        }

        let mut add_line = true;
        for rule in active_rules.iter_mut() {
            if rule.line_offset > n {
                continue;
            }

            if let Some(regex) = &rule.regex {
                if !rule.is_exclude {
                    for rule_match in regex.find_iter(&line) {
                        if !matches.contains_key(&rule.id) {
                            matches.insert(rule.id.clone(), vec![]);
                        }

                        let rule_matches = matches.get_mut(&rule.id).unwrap();

                        rule_matches.push(SearchResultMatch {
                            line: line_count,
                            start: rule_match.start(),
                            end: rule_match.end()
                        });
                    }
                }else {
                    if regex.is_match(&line) {
                        add_line = false;
                    }
                }
            }

        }
        if add_line {
            if !full_search {
                data.push_str(line);
                data.push_str("\n");
            }
            line_count += 1;
        }
    }

    Ok(SearchResultData {
        full_search,
        data,
        matches
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