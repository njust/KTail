use gtk::prelude::*;
use crate::file::toolbar::FileViewToolbar;
use std::path::PathBuf;
use gtk::{Orientation, WindowPosition, HeaderBar};
use crate::rules::{RuleListView};
use crate::file::file_view::{FileView};
use crate::{WorkbenchMsg, WorkbenchToolbarMsg};
use std::rc::Rc;

pub struct FileViewWorkbench {
    container: gtk::Box,
    rules_view: RuleListView,
    file_view: FileView,
    search_text: String,
    rules_dlg: Option<gtk::Dialog>,
    sender: Rc<dyn Fn(WorkbenchMsg)>
}

impl FileViewWorkbench {
    pub fn new<T: 'static + Clone + Fn(WorkbenchMsg)>(path: PathBuf, sender: T) -> Self  {
        let toolbar_msg = sender.clone();
        let toolbar = FileViewToolbar::new(move |msg| {
            toolbar_msg(WorkbenchMsg::ToolbarMsg(msg));
        });

        let file_view = FileView::new(path);
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let rule_msg = sender.clone();
        let rules_view = RuleListView::new(move |msg| {
            rule_msg(WorkbenchMsg::RuleMsg(msg));
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

    pub fn update(&mut self, msg: WorkbenchMsg) {
        match msg {
            WorkbenchMsg::ToolbarMsg(msg) => {
                match msg {
                    WorkbenchToolbarMsg::SearchPressed => {
                        self.file_view.search(self.search_text.clone());
                    }
                    WorkbenchToolbarMsg::ClearSearchPressed => {
                        self.file_view.clear_search(&self.search_text.clone());
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
            WorkbenchMsg::ApplyRules => {
                let rules = self.rules_view.get_rules();
                self.file_view.apply_rules(rules);
            }
            WorkbenchMsg::RuleMsg(msg) => {
                self.rules_view.update(msg);
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
                (*tx)(WorkbenchMsg::ApplyRules);
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