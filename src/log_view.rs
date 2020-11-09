use gtk::prelude::*;
use crate::toolbar::LogViewToolbar;
use gtk::{Orientation, WindowPosition, HeaderBar, AccelGroup};
use crate::highlighters::{HighlighterListView, SEARCH_ID, Highlighter};
use crate::log_text_view::{LogTextView};
use crate::model::{LogViewMsg, LogViewToolbarMsg, LogTextViewData, LogTextViewMsg};
use std::rc::Rc;
use uuid::Uuid;
use crate::util::{SortedListCompare, CompareResult};


pub struct LogView {
    container: gtk::Box,
    highlighters_view: HighlighterListView,
    log_text_view: LogTextView,
    search_text: String,
    highlighters_dlg: Option<gtk::Dialog>,
    highlighters: Vec<Highlighter>,
    toolbar: LogViewToolbar,
    active_rule: String,
    sender: Rc<dyn Fn(LogViewMsg)>
}

pub fn get_default_highlighters() -> Vec<Highlighter> {
    vec![
        Highlighter {
            id: Uuid::parse_str(SEARCH_ID).unwrap(),
            regex: None,
            color: Some(String::from("rgba(188,150,0,1)")),
            name: Some(String::from("Search")),
            is_system: true
        },
        Highlighter {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)error|fatal|failed(?-i))\s.*".into()),
            color: Some(String::from("rgba(239,41,41,1)")),
            name: Some(String::from("Error")),
            is_system: false
        },
        Highlighter {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)warn(?-i))\s.*".into()),
            color: Some(String::from("rgba(207,111,57,1)")),
            name: Some(String::from("Warning")),
            is_system: false
        }
    ]
}

impl LogView {
    pub fn new<T>(data: LogTextViewData, sender: T, accelerators: &AccelGroup) -> Self
        where T: 'static + Send + Clone + Fn(LogViewMsg)
    {
        let default_rules = get_default_highlighters();

        let toolbar_msg = sender.clone();
        let toolbar = LogViewToolbar::new(move |msg| {
            toolbar_msg(LogViewMsg::ToolbarMsg(msg));
        }, accelerators, &default_rules);

        let file_tx = sender.clone();
        let mut file_view = LogTextView::new();
        file_view.start(data, move |msg| {
            file_tx(LogViewMsg::LogTextViewMsg(msg));
        }, default_rules.clone());

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let rule_msg = sender.clone();
        let mut rules_view = HighlighterListView::new(move |msg| {
            rule_msg(LogViewMsg::HighlighterViewMsg(msg));
        });
        rules_view.add_highlighters(default_rules.clone());

        let mut rules = default_rules.clone();
        rules.sort_by_key(|r|r.id);

        Self {
            container,
            highlighters_view: rules_view,
            log_text_view: file_view,
            toolbar,
            search_text: String::new(),
            highlighters: rules,
            active_rule: SEARCH_ID.to_string(),
            highlighters_dlg: None,
            sender: Rc::new(sender.clone()),
        }
    }

    fn apply_rules(&mut self, mut rules: Vec<Highlighter>) {
        rules.sort_by_key(|r|r.id);
        let compare_results = SortedListCompare::new(&mut self.highlighters, &mut rules);
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
        self.highlighters = rules;
    }

    pub fn update(&mut self, msg: LogViewMsg) {
        match msg {
            LogViewMsg::ToolbarMsg(msg) => {
                match msg {
                    LogViewToolbarMsg::SearchPressed => {
                        let regex = if self.search_text.len() > 0 {
                            format!("(?i).*{}.*", self.search_text)
                        }else {
                            String::new()
                        };
                        self.highlighters_view.set_regex(SEARCH_ID, &regex);
                        if let Ok(rules) = self.highlighters_view.get_highlighter() {
                            self.apply_rules(rules.clone());
                            self.log_text_view.apply_rules(rules);
                        }
                    }
                    LogViewToolbarMsg::ClearSearchPressed => {
                        self.highlighters_view.set_regex(SEARCH_ID, &String::new());
                        if let Ok(rules) = self.highlighters_view.get_highlighter() {
                            self.apply_rules(rules.clone());
                            self.log_text_view.apply_rules(rules);
                        }
                    }
                    LogViewToolbarMsg::Clear  => {
                        self.toolbar.clear_counts();
                        self.log_text_view.clear_log();
                    }
                    LogViewToolbarMsg::ShowRules => {
                        self.show_dlg();
                    }
                    LogViewToolbarMsg::TextChange(text) => {
                        self.search_text = text;
                    }
                    LogViewToolbarMsg::ToggleAutoScroll(enable) => {
                        self.log_text_view.toggle_autoscroll(enable);
                    }
                    LogViewToolbarMsg::SelectNextMatch => {
                        self.log_text_view.select_next_match(&self.active_rule);
                    }
                    LogViewToolbarMsg::SelectPrevMatch => {
                        self.log_text_view.select_prev_match(&self.active_rule);
                    }
                    LogViewToolbarMsg::SelectRule(id) => {
                        self.log_text_view.set_active_rule(&id);
                        self.active_rule = id;
                    }
                }
            }
            LogViewMsg::ApplyRules => {
                if let Ok(rules) = self.highlighters_view.get_highlighter() {
                    self.apply_rules(rules.clone());
                    self.log_text_view.apply_rules(rules);
                }
            }
            LogViewMsg::HighlighterViewMsg(msg) => {
                self.highlighters_view.update(msg);
            }
            LogViewMsg::LogTextViewMsg(msg) => {
                if let LogTextViewMsg::Data(res) = &msg {
                    self.toolbar.update_results(&res.matches);
                }
                self.log_text_view.update(msg);
            }
        }
    }

    pub fn close(&mut self) {
        self.log_text_view.close();
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }

    pub fn show_dlg(&mut self) {
        if self.highlighters_dlg.is_none() {
            let dlg = gtk::Dialog::new();
            dlg.set_position(WindowPosition::CenterOnParent);
            dlg.set_default_size(400, 200);
            let header_bar = HeaderBar::new();
            header_bar.set_show_close_button(true);
            header_bar.set_title(Some("Highlighters"));
            dlg.set_titlebar(Some(&header_bar));
            dlg.set_modal(true);

            let content = dlg.get_content_area();
            content.add(self.highlighters_view.view());

            let tx = self.sender.clone();
            dlg.connect_delete_event(move |dlg, _| {
                (*tx)(LogViewMsg::ApplyRules);
                dlg.hide();
                gtk::Inhibit(true)
            });
            self.highlighters_dlg = Some(dlg);
        }

        if let Some(dlg)= &self.highlighters_dlg {
            dlg.show_all();
        }
    }
}