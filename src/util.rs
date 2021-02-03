use gtk::prelude::*;
use glib::{SignalHandlerId, Value, Sender};

use std::io::{BufReader, Read, BufRead};
use std::error::Error;
use encoding::all::{UTF_8};
use encoding::{DecoderTrap, Encoding};
use glib::bitflags::_core::cmp::Ordering;
use log::{error};

use std::collections::{HashMap};
use gtk::{TreeViewColumn, CellRendererText, CellRendererToggle, TreeStore, Widget, ApplicationWindow, DialogFlags, MessageType, ButtonsType};
use std::rc::Rc;
use crate::model::{ActiveRule, SearchResultData, SearchResultMatch, LogReplacer, Msg, RuleSearchResultData};


pub const APP_ICON_BUFFER: &'static [u8] = include_bytes!("../assets/app-icon/512x512.png");


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
    let mut rule_search_result: HashMap<String, RuleSearchResultData> = HashMap::new();

    let mut data = String::new();
    let mut line_count = 0;

    for (n, line) in lines {
        if line.len() <= 0 {
            continue;
        }

        let mut add_line = true;
        for rule in active_rules.iter_mut() {
            if rule.line_offset > n || (full_search && !rule.is_dirty) {
                continue;
            }

            if let Some(regex) = &rule.regex {
                if !rule.is_exclude {
                    for rule_match in regex.find_iter(&line) {
                        if !rule_search_result.contains_key(&rule.id) {
                            rule_search_result.insert(rule.id.clone(), RuleSearchResultData {
                                name: rule.name.clone(),
                                matches: vec![]
                            });
                        }

                        let rule_matches = rule_search_result.get_mut(&rule.id).unwrap();
                        let extracted_text = rule.extractor_regex.as_ref()
                            .and_then(|extractor| extractor.captures(&line))
                            .and_then(|extracted| extracted.name("text"))
                            .and_then(|extracted| Some(extracted.as_str().to_owned()));

                        rule_matches.matches.push(SearchResultMatch {
                            line: line_count,
                            start: rule_match.start(),
                            end: rule_match.end(),
                            extracted_text,
                            name: rule.name.clone()
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

    for active_rule in active_rules {
        active_rule.is_dirty = false;
    }

    Ok(SearchResultData {
        full_search,
        data,
        rule_search_result,
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
    Bool,
    Number
}

pub fn create_col(title: Option<&str>, idx: i32, col_type: ColumnType, ts: Rc<TreeStore>) -> TreeViewColumn {
    let col = TreeViewColumn::new();
    match col_type {
        ColumnType::String
        | ColumnType::Number => {
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

pub fn add_css<T: IsA<Widget>>(w: &T, css: &str) {
    let sc = w.get_style_context();
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_data(css.as_bytes()).expect("Could not load css from bytes");
    sc.add_provider(&css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
}

pub fn add_css_with_name<T: IsA<Widget>>(w: &T, widget_name: &str, css: &str) {
    w.set_widget_name(widget_name);
    add_css(w, css);
}

pub fn show_error_msg(msg: &str) {
    let dlg = gtk::MessageDialog::new::<ApplicationWindow>(
        None,
        DialogFlags::MODAL,
        MessageType::Error,
        ButtonsType::Ok,
        msg );
    dlg.run();
    dlg.close();
}

pub fn send_msg(tx: &Sender<Msg>, msg: Msg) {
    if let Err(e) = tx.send(msg) {
        error!("Could not send msg: {}", e);
    }
}

pub fn get_app_icon() -> gdk_pixbuf::Pixbuf {
    let app_icon_data = image::load_from_memory_with_format(
        APP_ICON_BUFFER,
        image::ImageFormat::Png,
    ).expect("Could not load app icon")
        .to_rgba8();

    gdk_pixbuf::Pixbuf::from_bytes(&glib::Bytes::from(app_icon_data.as_raw()), gdk_pixbuf::Colorspace::Rgb, true, 8, 512, 512, 512*4)
}