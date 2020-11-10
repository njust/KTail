use async_trait::async_trait;
use crate::highlighters::{Highlighter};
use regex::Regex;
use std::error::Error;
use uuid::Uuid;
use std::path::PathBuf;
use std::collections::HashMap;


pub struct CreateKubeLogData {
    pub pods: Vec<String>,
    pub since: u32,
}

pub enum LogTextViewData {
    File(PathBuf),
    Kube(CreateKubeLogData)
}

impl LogTextViewData {
    pub fn get_name(&self) -> String {
        match self {
            LogTextViewData::File(file_path) => file_path.file_name().unwrap().to_str().unwrap().to_string(),
            LogTextViewData::Kube(data) => data.pods.join(",")
        }
    }
}


#[derive(Debug)]
pub struct SearchResultData {
    pub full_search: bool,
    pub data: String,
    pub matches: HashMap<String, Vec<SearchResultMatch>>,
}

pub enum Msg {
    CloseTab(Uuid),
    CreateTab(LogTextViewData),
    NextTab,
    PrevTab,
    CloseActiveTab,
    WorkbenchMsg(Uuid, LogViewMsg),
    Exit
}

pub enum LogViewMsg {
    ApplyRules,
    ToolbarMsg(LogViewToolbarMsg),
    HighlighterViewMsg(RuleListViewMsg),
    LogTextViewMsg(LogTextViewMsg)
}

pub enum LogViewToolbarMsg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    ShowRules,
    ToggleAutoScroll(bool),
    SelectNextMatch,
    SelectPrevMatch,
    SelectRule(String),
    Clear
}

pub enum RuleListViewMsg {
    AddRule(Highlighter),
    DeleteRule(String),
}

#[derive(Debug, Clone)]
pub struct SearchResultMatch {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub enum LogTextViewMsg {
    Data(SearchResultData),
    Clear,
    CursorChanged,
}

pub enum LogState {
    Ok,
    Skip,
    Reload
}

#[async_trait]
pub trait LogReader : std::marker::Send {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    async fn init(&mut self);
    fn check_changes(&mut self) -> LogState;
    fn stop(&mut self);
}

pub struct ActiveRule {
    pub id: String,
    pub line_offset: usize,
    pub regex: Option<Regex>,
    pub is_exclude: bool,
}

pub struct RuleChanges {
    pub add: Vec<Highlighter>,
    pub remove: Vec<String>,
    pub update: Vec<Highlighter>,
    pub data: Option<String>,
}

pub enum LogTextViewThreadMsg {
    ApplyRules(RuleChanges),
    Quit,
}

pub struct LogReplacer<'a> {
    pub regex: Regex,
    pub replace_with: &'a str,
}