use gtk::prelude::*;
use crate::toolbar::LogViewToolbar;
use gtk::{Orientation, WindowPosition, AccelGroup, HeaderBarBuilder};
use crate::highlighters::{HighlighterListView, SEARCH_ID};
use crate::log_text_view::{LogTextView};
use crate::model::{LogViewMsg, LogViewToolbarMsg, CreateLogView};
use std::rc::Rc;
use crate::get_default_highlighters;


pub struct LogView {
    container: gtk::Box,
    highlighters_view: HighlighterListView,
    log_text_view: LogTextView,
    search_text: String,
    highlighters_dlg: Option<gtk::Dialog>,
    sender: Rc<dyn Fn(LogViewMsg)>,
}

impl LogView {
    pub fn new<T>(mut data: CreateLogView, sender: T, accelerators: &AccelGroup) -> Self
        where T: 'static + Send + Clone + Fn(LogViewMsg)
    {
        let default_rules = data.rules.take().unwrap_or(get_default_highlighters());

        let toolbar_msg = sender.clone();
        let toolbar = LogViewToolbar::new(move |msg| {
            toolbar_msg(LogViewMsg::ToolbarMsg(msg));
        }, accelerators);

        let file_tx = sender.clone();
        let tm = sender.clone();
        let mut file_view = LogTextView::new(accelerators, move |msg| {
            tm(LogViewMsg::LogTextViewMsg(msg));
        });

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
            search_text: String::new(),
            highlighters_dlg: None,
            sender: Rc::new(sender.clone()),
        }
    }


    pub fn update(&mut self, msg: LogViewMsg) {
        match msg {
            LogViewMsg::ToolbarMsg(msg) => {
                match msg {
                    LogViewToolbarMsg::SearchPressed => {
                        let (regex, extractor) = if self.search_text.len() > 0 {
                            (
                                format!("(?i).*{}.*", self.search_text),
                                format!(r"(?P<text>(?i){}(?-i).*)", self.search_text),
                            )
                        }else {
                            (String::new(), String::new())
                        };

                        self.highlighters_view.set_regex(SEARCH_ID, &regex, &extractor);
                        if let Ok(rules) = self.highlighters_view.get_highlighter() {
                            self.log_text_view.apply_rules(rules);
                        }
                    }
                    LogViewToolbarMsg::ClearSearchPressed => {
                        self.highlighters_view.set_regex(SEARCH_ID, &String::new(), &String::new());
                        if let Ok(rules) = self.highlighters_view.get_highlighter() {
                            self.log_text_view.apply_rules(rules);
                        }
                    }
                    LogViewToolbarMsg::Clear  => {
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
                }
            }
            LogViewMsg::ApplyRules => {
                if let Ok(rules) = self.highlighters_view.get_highlighter() {
                    self.log_text_view.apply_rules(rules);
                }
            }
            LogViewMsg::LogTextViewMsg(msg) => {
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