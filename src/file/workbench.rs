use gtk::prelude::*;
use crate::file::toolbar::FileViewToolbar;
use gtk::{Orientation, WindowPosition, HeaderBar, AccelGroup};
use crate::rules::{RuleListView, SEARCH_ID, Rule};
use crate::file::file_view::{FileView};
use crate::{WorkbenchViewMsg, WorkbenchToolbarMsg, FileViewData, FileViewMsg};
use std::rc::Rc;
use uuid::Uuid;
use crate::util::{SortedListCompare, CompareResult};


pub struct FileViewWorkbench {
    container: gtk::Box,
    rules_view: RuleListView,
    file_view: FileView,
    search_text: String,
    rules_dlg: Option<gtk::Dialog>,
    rules: Vec<Rule>,
    toolbar: FileViewToolbar,
    active_rule: String,
    sender: Rc<dyn Fn(WorkbenchViewMsg)>
}

pub fn get_default_rules() -> Vec<Rule> {
    vec![
        Rule {
            id: Uuid::parse_str(SEARCH_ID).unwrap(),
            regex: None,
            color: Some(String::from("rgba(229,190,90,1)")),
            name: Some(String::from("Search")),
            is_system: true
        },
        Rule {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)error|fatal|failed(?-i))\s.*".into()),
            color: Some(String::from("rgba(255,96,102,1)")),
            name: Some(String::from("Error")),
            is_system: false
        },
        Rule {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)warn(?-i))\s.*".into()),
            color: Some(String::from("rgba(207,111,57,1)")),
            name: Some(String::from("Warning")),
            is_system: false
        }
    ]
}

impl FileViewWorkbench {
    pub fn new<T>(data: FileViewData, sender: T, accelerators: &AccelGroup) -> Self
        where T: 'static + Send + Clone + Fn(WorkbenchViewMsg)
    {
        let default_rules = get_default_rules();

        let toolbar_msg = sender.clone();
        let toolbar = FileViewToolbar::new(move |msg| {
            toolbar_msg(WorkbenchViewMsg::ToolbarMsg(msg));
        }, accelerators, &default_rules);

        let file_tx = sender.clone();
        let mut file_view = FileView::new();
        file_view.start(data, move |msg| {
            file_tx(WorkbenchViewMsg::FileViewMsg(msg));
        }, default_rules.clone());

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let rule_msg = sender.clone();
        let mut rules_view = RuleListView::new(move |msg| {
            rule_msg(WorkbenchViewMsg::RuleViewMsg(msg));
        });
        rules_view.add_rules(default_rules.clone());

        let mut rules = default_rules.clone();
        rules.sort_by_key(|r|r.id);

        Self {
            container,
            rules_view,
            file_view,
            toolbar,
            search_text: String::new(),
            rules,
            active_rule: SEARCH_ID.to_string(),
            rules_dlg: None,
            sender: Rc::new(sender.clone()),
        }
    }

    fn apply_rules(&mut self, mut rules: Vec<Rule>) {
        rules.sort_by_key(|r|r.id);
        let compare_results = SortedListCompare::new(&mut self.rules, &mut rules);
        for compare_result in compare_results {
            match compare_result {
                CompareResult::MissesLeft(new) => {
                    self.toolbar.add_rule(new);
                }
                CompareResult::MissesRight(delete) => {
                    self.toolbar.delete_rule(&delete.id.to_string());
                }
                CompareResult::Equal(left, right) => {
                    if let Some(iter) = self.toolbar.get_rule_iter(&left.id.to_string()) {
                        if left.regex != right.regex {
                            self.toolbar.clear_counts();
                        }

                        if left.name != right.name {
                            let default = String::from("Unamed rule");
                            let name = right.name.as_ref().unwrap_or(&default);
                            self.toolbar.update_rule(&iter, &name);
                        }
                    }
                }
            }
        }
        self.rules = rules;
    }

    pub fn update(&mut self, msg: WorkbenchViewMsg) {
        match msg {
            WorkbenchViewMsg::ToolbarMsg(msg) => {
                match msg {
                    WorkbenchToolbarMsg::SearchPressed => {
                        let regex = if self.search_text.len() > 0 {
                            format!("(?i){}", self.search_text)
                        }else {
                            String::new()
                        };
                        self.rules_view.set_regex(SEARCH_ID, &regex);
                        let rules = self.rules_view.get_rules();
                        self.apply_rules(rules.clone());
                        self.file_view.apply_rules(rules);
                    }
                    WorkbenchToolbarMsg::ClearSearchPressed => {
                        self.rules_view.set_regex(SEARCH_ID, &String::new());
                        let rules = self.rules_view.get_rules();
                        self.apply_rules(rules.clone());
                        self.file_view.apply_rules(rules);
                    }
                    WorkbenchToolbarMsg::ShowRules => {
                        self.show_dlg();
                    }
                    WorkbenchToolbarMsg::TextChange(text) => {
                        self.search_text = text;
                    }
                    WorkbenchToolbarMsg::ToggleAutoScroll(enable) => {
                        self.file_view.toggle_autoscroll(enable);
                    }
                    WorkbenchToolbarMsg::SelectNextMatch => {
                        self.file_view.select_next_match(&self.active_rule);
                    }
                    WorkbenchToolbarMsg::SelectPrevMatch => {
                        self.file_view.select_prev_match(&self.active_rule);
                    }
                    WorkbenchToolbarMsg::SelectRule(id) => {
                        self.file_view.set_active_rule(&id);
                        self.active_rule = id;
                    }
                }
            }
            WorkbenchViewMsg::ApplyRules => {
                let rules = self.rules_view.get_rules();
                self.apply_rules(rules.clone());
                self.file_view.apply_rules(rules);
            }
            WorkbenchViewMsg::RuleViewMsg(msg) => {
                self.rules_view.update(msg);
            }
            WorkbenchViewMsg::FileViewMsg(msg) => {
                if let FileViewMsg::Data(_length, _data, matches) = &msg {
                    self.toolbar.update_results(matches);
                }
                self.file_view.update(msg);
            }
        }
    }

    pub fn close(&mut self) {
        self.file_view.close();
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }

    pub fn show_dlg(&mut self) {
        if self.rules_dlg.is_none() {
            let dlg = gtk::Dialog::new();
            dlg.set_position(WindowPosition::CenterOnParent);
            dlg.set_default_size(400, 200);
            let header_bar = HeaderBar::new();
            header_bar.set_show_close_button(true);
            header_bar.set_title(Some("Highlighters"));
            dlg.set_titlebar(Some(&header_bar));
            dlg.set_modal(true);

            let content = dlg.get_content_area();
            content.add(self.rules_view.view());

            let tx = self.sender.clone();
            dlg.connect_delete_event(move |dlg, _| {
                (*tx)(WorkbenchViewMsg::ApplyRules);
                dlg.hide();
                gtk::Inhibit(true)
            });
            self.rules_dlg = Some(dlg);
        }

        if let Some(dlg)= &self.rules_dlg {
            dlg.show_all();
        }
    }
}


impl Drop for FileViewWorkbench {
    fn drop(&mut self) {
        println!("Drop workbench");
    }
}