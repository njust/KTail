use std::cmp::Ordering;
use std::sync::mpsc::Sender;
use gtk4_helper::prelude::MsgHandler;
use regex::Regex;
use crate::log_stream::LogData;
use crate::log_view::{HighlightResultData, SearchData, SearchResultData};
use crate::LogViewMsg;
use crate::util::search_offset;

struct LogDataCache {
    timestamp: i64,
    data: Option<LogData>,
}

impl PartialEq<Self> for LogDataCache {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp.eq(&other.timestamp)
    }
}

impl PartialOrd for LogDataCache {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.timestamp.partial_cmp(&other.timestamp)
    }
}

impl From<i64> for LogDataCache {
    fn from(timestamp: i64) -> Self {
        Self {
            timestamp,
            data: None
        }
    }
}

pub enum WorkerData {
    ProcessLogData(Vec<LogData>),
    ProcessHighlighters(Vec<SearchData>, LogData, String),
    Clear,
    GetOffsetForTimestamp(i64),
    Search(Regex),
}

pub fn start_worker<T: MsgHandler<LogViewMsg>>(tx: T) -> Sender<WorkerData> {
    let (w_tx, w_rx) = std::sync::mpsc::channel::<WorkerData>();
    std::thread::spawn(move || {
        let mut ordered_log_data: Vec<LogDataCache> = vec![];
        while let Ok(data) = w_rx.recv() {
            match data {
                WorkerData::ProcessLogData(data) => {
                    let mut res = vec![];
                    for datum in data {
                        let timestamp = datum.timestamp.timestamp_nanos();
                        let mut offset = search_offset(&ordered_log_data, timestamp.into());
                        let len = ordered_log_data.len();
                        // Make sure we append data with the same timestamp to the bottom
                        while offset < len && ordered_log_data[offset].timestamp == timestamp {
                            offset += 1;
                        }
                        ordered_log_data.insert(offset, LogDataCache {
                            timestamp,
                            data: Some(datum.clone())
                        });
                        // We need to insert a extra entry for lines starting with a linefeed or a new line
                        if datum.text.starts_with("\r") || datum.text.starts_with("\n") {
                            // Sourceview seems to ignore those
                            if datum.text != "\r\n" && datum.text != "\n" {
                                ordered_log_data.insert(offset, timestamp.into());
                            }
                        }
                        res.push((offset as i64, datum));
                    }

                    tx(LogViewMsg::LogDataProcessed(res))
                }
                WorkerData::Clear => {
                    ordered_log_data.clear();
                }
                WorkerData::ProcessHighlighters(highlighters, data, text_marker_id) => {
                    let mut res = HighlightResultData {
                        text_marker_id,
                        timestamp: data.timestamp,
                        matching_highlighters: vec![]
                    };

                    for highlighter in highlighters {
                        if highlighter.search.is_match(&data.text) {
                            res.matching_highlighters.push(highlighter.name)
                        }
                    }
                    tx(LogViewMsg::HighlightResult(res));
                }
                WorkerData::GetOffsetForTimestamp(timestamp) => {
                    let ts = timestamp * 1000 * 1000 * 1000; // Seconds to nanoseconds
                    let offset = search_offset(&ordered_log_data, ts.into());
                    tx(LogViewMsg::ScrollToLine(offset as i64));
                }
                WorkerData::Search(query) => {
                    let mut search_results = SearchResultData::new();
                    for (idx, data)  in ordered_log_data.iter().enumerate() {
                        if let Some(data) = &data.data {
                            if query.is_match(&data.text) {
                                search_results.lines.push((idx, data.clone()));
                            }
                        }
                    }
                    tx(LogViewMsg::SearchResult(search_results))
                }
            }
        }
    });
    w_tx
}