use gtk::prelude::*;
use crate::file_view::{FileView};
use crate::file_view::toolbar::FileViewToolbar;
use std::rc::Rc;
use glib::bitflags::_core::cell::RefCell;
use std::path::PathBuf;
use gtk::{Orientation, TreePath, TreeIter};
use crate::file_view::rules::{RuleList, CustomRule, RulesDialog, RuleListView};
use uuid::Uuid;

pub enum RuleMsg {
    AddRule(CustomRule),
    DeleteRule(Uuid),
    NameChanged(Uuid, String),
    RegexChanged (Uuid, String),
    ColorChanged(Uuid, String),
}

pub enum Msg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    RuleMsg(RuleMsg),
    ApplyRules,
    ToggleAutoScroll(bool),
    ShowRules
}

pub struct WorkbenchState {
    search_text: String,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            search_text: String::new(),
        }
    }
}

pub struct FileViewWorkbench {
    container: gtk::Box,
}

impl FileViewWorkbench {
    pub fn new(path: PathBuf) -> Self  {
        let state = Rc::new(RefCell::new(WorkbenchState::default()));
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let toolbar = FileViewToolbar::new(tx.clone());

        let mut file_view = FileView::new(path);
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(toolbar.view());
        container.add(file_view.view());

        let mut rules = RuleListView::new(tx.clone());
        let dlg = RulesDialog::new(&rules, tx.clone());
        rx.attach(None, move |msg| {
            match msg {
                Msg::SearchPressed => {
                    file_view.search(state.borrow().search_text.clone());
                }
                Msg::ClearSearchPressed => {
                    let current_search = &state.borrow().search_text;
                    file_view.clear_search(current_search);
                }
                Msg::TextChange(text) => {
                    state.borrow_mut().search_text = text;
                }
                Msg::ToggleAutoScroll(enable) => {
                    file_view.toggle_autoscroll(enable)
                }
                Msg::ShowRules => {
                    dlg.show();
                }
                Msg::ApplyRules => {
                    let rules = rules.get_rules();
                    file_view.apply_rules(rules);
                }
                Msg::RuleMsg(msg) => {
                    rules.update(msg);
                }
            }
            glib::Continue(true)
        });

        Self {
            container
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}