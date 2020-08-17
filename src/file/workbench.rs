use gtk::prelude::*;
use crate::file::toolbar::FileViewToolbar;
use std::path::PathBuf;
use gtk::{Orientation, WindowPosition, HeaderBar};
use crate::rules::{RuleListView, SEARCH_ID, Rule};
use crate::file::file_view::{FileView};
use crate::{WorkbenchViewMsg, WorkbenchToolbarMsg};
use std::rc::Rc;
use uuid::Uuid;

pub struct FileViewWorkbench {
    container: gtk::Box,
    rules_view: RuleListView,
    file_view: FileView,
    search_text: String,
    rules_dlg: Option<gtk::Dialog>,
    sender: Rc<dyn Fn(WorkbenchViewMsg)>
}

impl FileViewWorkbench {
    pub fn new<T>(path: PathBuf, sender: T) -> Self
        where T: 'static + Send + Clone + Fn(WorkbenchViewMsg)
    {
        let toolbar_msg = sender.clone();
        let toolbar = FileViewToolbar::new(move |msg| {
            toolbar_msg(WorkbenchViewMsg::ToolbarMsg(msg));
        });

        let file_tx = sender.clone();
        let file_view = FileView::new(path, move |msg| {
            file_tx(WorkbenchViewMsg::FileViewMsg(msg));
        });
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let rule_msg = sender.clone();
        let mut rules_view = RuleListView::new(move |msg| {
            rule_msg(WorkbenchViewMsg::RuleViewMsg(msg));
        });

        &rules_view.add_rule(Rule {
            id: Uuid::parse_str(SEARCH_ID).unwrap(),
            regex: None,
            color: Some(String::from("rgba(229,190,90,1)")),
            name: Some(String::from("Search")),
            is_system: true
        });

        Self {
            container,
            rules_view,
            file_view,
            search_text: String::new(),
            rules_dlg: None,
            sender: Rc::new(sender.clone()),
        }
    }

    pub fn update(&mut self, msg: WorkbenchViewMsg) {
        match msg {
            WorkbenchViewMsg::ToolbarMsg(msg) => {
                match msg {
                    WorkbenchToolbarMsg::SearchPressed => {
                        if let Some(rule_view) = self.rules_view.get_rule_by_id(SEARCH_ID) {
                            let regex = if self.search_text.len() > 0 {
                                Some(self.search_text.clone())
                            } else {
                                None
                            };
                            rule_view.set_regex(regex);
                        }
                        let rules = self.rules_view.get_rules();
                        self.file_view.apply_rules(rules);
                    }
                    WorkbenchToolbarMsg::ClearSearchPressed => {
                        if let Some(rule_view) = self.rules_view.get_rule_by_id(SEARCH_ID) {
                            rule_view.set_regex(None);
                        }
                        let rules = self.rules_view.get_rules();
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
                }
            }
            WorkbenchViewMsg::ApplyRules => {
                let rules = self.rules_view.get_rules();
                self.file_view.apply_rules(rules);
            }
            WorkbenchViewMsg::RuleViewMsg(msg) => {
                self.rules_view.update(msg);
            }
            WorkbenchViewMsg::FileViewMsg(msg) => {
                self.file_view.update(msg);
            }
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }

    pub fn show_dlg(&mut self) {
        if self.rules_dlg.is_none() {
            let dlg = gtk::Dialog::new();
            dlg.set_position(WindowPosition::Mouse);
            dlg.set_default_size(400, 200);
            let header_bar = HeaderBar::new();
            header_bar.set_show_close_button(true);
            header_bar.set_title(Some("Rules"));
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