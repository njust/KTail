use async_trait::async_trait;
use crate::highlighters::{Highlighter};
use regex::Regex;
use std::error::Error;
use uuid::Uuid;
use std::path::PathBuf;
use std::collections::HashMap;

pub struct CreateKubeLogData {
    pub pods: Vec<String>,
    pub cluster: String,
    pub namespace: String,
    pub since: u32,
}

pub struct CreateLogView {
    pub rules: Option<Vec<Highlighter>>,
    pub data: LogViewData,
}

impl CreateLogView {
    pub fn new(data: LogViewData) -> Msg {
        Msg::CreateTab(Self {
            data,
            rules: None,
        })
    }

    pub fn with_rules(data: LogViewData, rules: Vec<Highlighter>) -> Msg {
        Msg::CreateTab(
        Self {
            data,
            rules: Some(rules),
        })
    }
}

pub enum LogViewData {
    File(PathBuf),
    Kube(CreateKubeLogData)
}

impl LogViewData {
    pub fn get_name(&self) -> String {
        match self {
            LogViewData::File(file_path) => file_path.file_name().unwrap().to_str().unwrap().to_string(),
            LogViewData::Kube(data) => {
                format!("{} > {} > {}", data.cluster, data.namespace, data.pods.join(", "))
            }
        }
    }
}


#[derive(Debug)]
pub struct SearchResultData {
    pub full_search: bool,
    pub data: String,
    pub rule_search_result: HashMap<String, RuleSearchResultData>,
}

pub enum PodSelectorMsg {
    Show,
    ToggleIncludeReplicas(bool),
    ToggleSeparateTabs(bool),
    SinceUnitChanged(String),
    SinceChanged(u32),
    ClusterChanged(String),
    NamespaceChanged(String),
    Ok,
    Close,
}

pub enum Msg {
    CloseTab(Uuid),
    CreateTab(CreateLogView),
    NextTab,
    PrevTab,
    CloseActiveTab,
    LogViewMsg(Uuid, LogViewMsg),
    PodSelectorMsg(PodSelectorMsg),
    Exit
}

pub enum LogViewMsg {
    ApplyRules,
    ToolbarMsg(LogViewToolbarMsg),
    LogTextViewMsg(LogTextViewMsg)
}

pub enum LogViewToolbarMsg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    ShowRules,
    ToggleAutoScroll(bool),
    Clear
}

#[derive(Debug)]
pub enum ExtractSelection {
    SearchGroup(String),
    TextGroup(String, u32)
}

#[derive(Debug)]
pub enum LogTextViewMsg {
    Data(SearchResultData),
    Clear,
    CursorChanged,
    ToggleBookmark(u16),
    ScrollToBookmark(u16),
    ExtractSelected(ExtractSelection),
    NextMatch,
    PrevMatch
}

#[derive(Debug, Clone)]
pub struct SearchResultMatch {
    pub name: Option<String>,
    pub line: usize,
    pub start: usize,
    pub end: usize,
    pub extracted_text: Option<String>,
}

#[derive(Debug)]
pub struct RuleSearchResultData {
    pub name: Option<String>,
    pub matches: Vec<SearchResultMatch>,
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
    pub name: Option<String>,
    pub line_offset: usize,
    pub regex: Option<Regex>,
    pub extractor_regex: Option<Regex>,
    pub is_exclude: bool,
    pub is_dirty: bool,
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