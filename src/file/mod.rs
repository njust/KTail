pub mod workbench;
pub mod toolbar;
pub mod file_view;
use crate::rules::{CustomRule, Rule};

pub struct SearchResultMatch {
    pub line: usize,
    pub start: usize,
    pub end: usize,
}

struct SearchResult {
    tag: String,
    with_offset: bool,
    matches: Vec<SearchResultMatch>,
}

struct ActiveRule {
    is_new: bool,
    rule: Rule,
}

enum FileUiMsg {
    Data(u64, String, Vec<SearchResult>),
    Clear,
}

struct RuleChanges {
    add: Vec<CustomRule>,
    remove: Vec<String>,
    update: Vec<CustomRule>,
}

enum FileThreadMsg {
    AddRule(Rule),
    DeleteRule(Rule),
    ApplyRules(RuleChanges),
}
