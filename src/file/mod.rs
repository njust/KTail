pub mod workbench;
pub mod toolbar;
pub mod file_view;
use crate::rules::{CustomRule, Rule};


struct ActiveRule {
    is_new: bool,
    rule: Rule,
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
