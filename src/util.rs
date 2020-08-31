use gtk::prelude::*;
use glib::{SignalHandlerId, Value};
use std::path::PathBuf;
use std::io::{BufReader, SeekFrom, Read, Seek, BufRead};
use std::error::Error;
use encoding::all::{UTF_8, UTF_16LE, UTF_16BE};
use encoding::{DecoderTrap};
use glib::bitflags::_core::cmp::Ordering;
use crate::{SearchResultMatch, TaggedSearchResult};
use serde::Deserialize;
use std::process::{Command, Stdio};
use std::collections::HashSet;
use gtk::{TreeViewColumn, CellRendererText, CellRendererToggle, TreeStore};
use std::rc::Rc;
use crate::file::ActiveRule;

pub const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn enable_auto_scroll(text_view : &sourceview::View) -> SignalHandlerId {
    text_view.connect_size_allocate(|tv, _b| {
        if let Some(buffer) = tv.get_buffer() {
            let mut end = buffer.get_end_iter();
            tv.scroll_to_iter(&mut end,  0.0, true, 0.5, 0.5);
        }
    })
}

pub fn get_encoding(bytes: &[u8]) -> &'static dyn encoding::types::Encoding {
    if bytes.len() <= 2 {
        return UTF_8;
    }

    // https://de.wikipedia.org/wiki/Byte_Order_Mark
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

pub struct SearchResult {
    pub lines: usize,
    pub matches: Vec<SearchResultMatch>,
}

pub fn decode_data(buffer: &[u8], encoding: Option<&'static dyn encoding::types::Encoding>) -> Result<ReadResult, Box<dyn Error>> {
    let encoding = if let Some(encoding) = encoding {
        encoding
    }else {
        get_encoding(&buffer)
    };

    let mut data = encoding.decode(buffer, DecoderTrap::Ignore)?;
    // let re = Regex::new("\n\r|\r\n|\r")?;
    // data = re.replace_all(&data, "").to_string();
    data = data.replace("\n\r", "\n");
    data = data.replace("\r\n", "\n");
    data = data.replace("\r", "\n");
    data = data.replace("", "");


    let read_bytes = buffer.len() as u64;
    Ok(ReadResult {
        read_bytes,
        data,
        encoding: if read_bytes > 0 {Some(encoding) } else {None}
    })
}

pub fn read_file(path: &PathBuf, start: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = std::fs::File::open(path)?;

    let mut reader = BufReader::new(file);
    let mut buffer = vec![];
    if start > 0 {
        reader.seek(SeekFrom::Start(start))?;
    }

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

pub struct SearchResultData {
    pub lines: usize,
    pub results: Vec<TaggedSearchResult>,
}

pub fn search(text: &str, active_rules: &mut Vec<ActiveRule>, line_offset: usize) -> Result<SearchResultData, Box<dyn Error>> {
    let mut re_list_matches = vec![];
    let lines = text.split("\n").enumerate();
    let mut line_cnt = 0;

    for (n, line) in lines {
        line_cnt = n;
        for search_data in active_rules.iter_mut() {
            if search_data.line_offset > n {
                continue;
            }

            let mut matches = vec![];
            if let Some(regex) = &search_data.regex {
                if text.len() > 0 {
                    for mat in regex.find_iter(&line) {
                        matches.push(SearchResultMatch {
                            line: n + line_offset,
                            start: mat.start(),
                            end: mat.end()
                        });
                    }

                    re_list_matches.push(TaggedSearchResult {
                        matches,
                        tag: search_data.id.clone()
                    })
                }
            }

        }
    }

    Ok(SearchResultData {
        lines: line_cnt,
        results: re_list_matches
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

#[derive(Deserialize)]
struct PodContainer {
    name: String,
}

#[derive(Deserialize)]
struct PodSpec {
    containers: Vec<PodContainer>,
}

#[derive(Deserialize)]
struct PodItems {
    spec: PodSpec,
}

#[derive(Deserialize)]
struct GetPodsResult {
    items: Vec<PodItems>
}


#[test]
pub fn test_get_pods() {
    if let Ok(p) = get_pods() {
        println!("P: {:?}", p);
    }
}

pub fn kubectl_file_name() -> &'static str {
    #[cfg(target_family = "windows")]
        let bin = "kubectl.exe";
    #[cfg(target_family = "unix")]
        let bin = "kubectl";
    bin
}

pub fn kubectl_in_path() -> bool {
    let bin = kubectl_file_name();
    is_file_in_path(bin)
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

pub fn get_pods() -> Result<Vec<String>, Box<dyn Error>> {
    let bin = kubectl_file_name();

    let mut cmd = Command::new(bin);
    #[cfg(target_family = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let cmd = cmd
        .stdout(Stdio::piped())
        .arg("get")
        .arg("pods")
        .arg("-o")
        .arg("json")
        .spawn()?;

    let mut names = HashSet::new();
    if let Some(mut out)=cmd.stdout {
        let mut data = String::new();
        out.read_to_string(&mut data)?;
        let res = serde_json::from_str::<GetPodsResult>(&data)?;
        for item in res.items {
            if let Some(f) = &item.spec.containers.first() {
                names.insert(f.name.clone());
            }
        }
    }
    Ok(names.into_iter().collect())
}

pub fn is_file_in_path(file_name: &str) -> bool {
    #[cfg(target_family = "windows")]
    let separator = ";";
    #[cfg(target_family = "unix")]
    let separator = ":";

    if let Ok(path) = std::env::var("PATH") {
        let seg = path.split(separator);
        for current_path in seg {
            let file_path = std::path::Path::new(current_path).join(file_name);
            if file_path.exists() {
                return true;
            }
        }
    }
    return false;
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