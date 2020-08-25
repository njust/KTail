pub mod workbench;
pub mod toolbar;
pub mod file_view;
use crate::rules::{Rule};


struct ActiveRule {
    line_offset: usize,
    rule: Rule,
}

struct RuleChanges {
    add: Vec<Rule>,
    remove: Vec<String>,
    update: Vec<Rule>,
}

enum FileThreadMsg {
    ApplyRules(RuleChanges),
}
