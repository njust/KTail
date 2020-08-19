use gtk::prelude::*;
use glib::{SignalHandlerId};
use std::path::PathBuf;
use std::io::{BufReader, SeekFrom, Read, Seek};
use std::error::Error;
use encoding::all::{UTF_8, UTF_16LE, UTF_16BE};
use encoding::{DecoderTrap};
use glib::bitflags::_core::cmp::Ordering;
use regex::Regex;
use crate::SearchResultMatch;

pub fn enable_auto_scroll(text_view : &sourceview::View) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv, _b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

pub fn get_encoding(bytes: &Vec<u8>) -> &'static dyn encoding::types::Encoding {
    if bytes.len() <= 2 {
        return UTF_8;
    }

    let bom = &bytes[0..2];
    match &bom {
        &[239u8, 187u8] => UTF_8,
        &[254u8, 255u8] => UTF_16BE,
        &[255u8, 254u8] => UTF_16LE,
        _ => UTF_8
    }
}

pub struct ReadResult {
    pub read_bytes: u64,
    pub data: String,
    pub encoding: Option<&'static dyn encoding::types::Encoding>
}

pub fn read_file(path: &PathBuf, start: u64, encoding: Option<&'static dyn encoding::types::Encoding>) -> Result<ReadResult, Box<dyn Error>> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    if start > 0 {
        reader.seek(SeekFrom::Start(start))?;
    }

    let read_bytes = reader.read_to_end(&mut buffer)?;
    let encoding = if let Some(encoding) = encoding {
        encoding
    }else {
        get_encoding(&buffer)
    };

    let data = encoding.decode(buffer.as_slice(), DecoderTrap::Replace)?;
    Ok(ReadResult {
        read_bytes: read_bytes as u64,
        data,
        encoding: Some(encoding)
    })
}

pub fn search(text: &str, search: &str) -> Result<Vec<SearchResultMatch>, Box<dyn Error>> {
    let lines = text.split("\n");
    let re = Regex::new(search)?;
    let mut matches = vec![];
    for (n, line) in lines.enumerate() {
        for mat in re.find_iter(&line) {
            matches.push(SearchResultMatch {
                line: n,
                start: mat.start(),
                end: mat.end()
            });
        }
    }
    Ok(matches)
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