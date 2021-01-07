use gtk::prelude::*;
use crate::toolbar::LogViewToolbar;
use gtk::{Orientation, WindowPosition, AccelGroup, HeaderBarBuilder};
use crate::highlighters::{HighlighterListView, Highlighter, SEARCH_ID, RULE_TYPE_HIGHLIGHT};
use crate::log_text_view::{LogTextView};
use crate::model::{LogViewMsg, LogViewToolbarMsg, LogTextViewMsg, UNNAMED_RULE, CreateLogView};
use std::rc::Rc;
use crate::util::{SortedListCompare, CompareResult};
use crate::get_default_highlighters;


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

impl LogView {
    pub fn new<T>(mut data: CreateLogView, sender: T, accelerators: &AccelGroup) -> Self
        where T: 'static + Send + Clone + Fn(LogViewMsg)
    {
        let default_rules = data.rules.take().unwrap_or(get_default_highlighters());

        let toolbar_msg = sender.clone();
        let toolbar = LogViewToolbar::new(move |msg| {
            toolbar_msg(LogViewMsg::ToolbarMsg(msg));
        }, accelerators, &default_rules);

        let file_tx = sender.clone();
        let mut file_view = LogTextView::new();
        file_view.start(data.data, move |msg| {
            file_tx(LogViewMsg::LogTextViewMsg(msg));
        }, default_rules.clone());

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let rules_view = HighlighterListView::new();
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

    fn apply_rules(&mut self, rules: Vec<Highlighter>) {
        let mut rules = rules.into_iter().filter(|r| r.rule_type == RULE_TYPE_HIGHLIGHT ).collect::<Vec<Highlighter>>();
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
                            let default = String::from(UNNAMED_RULE);
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
            let header_bar = HeaderBarBuilder::new()
                .show_close_button(true)
                .title("Rules")
                .build();

            let dlg = gtk::DialogBuilder::new()
                .window_position(WindowPosition::CenterOnParent)
                .default_width(400)
                .default_height(200)
                .modal(true)
                .build();
            dlg.set_titlebar(Some(&header_bar));

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