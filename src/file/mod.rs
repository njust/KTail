pub mod workbench;
pub mod toolbar;
pub mod file_view;
use crate::rules::{Rule};
use regex::Regex;


pub struct ActiveRule {
    pub id: String,
    pub line_offset: usize,
    // pub rule: Rule,
    pub regex: Option<Regex>,
}

struct RuleChanges {
    add: Vec<Rule>,
    remove: Vec<String>,
    update: Vec<Rule>,
}

enum FileThreadMsg {
    ApplyRules(RuleChanges),
}
