use gtk::prelude::*;
use crate::file_view::{FileView};
use crate::file_view::toolbar::FileViewToolbar;
use std::rc::Rc;
use glib::bitflags::_core::cell::RefCell;
use std::path::PathBuf;
use gtk::{Orientation, TreePath};
use crate::file_view::rules::{RuleList, CustomRule, RulesDialog};

pub enum RuleMsg {
    ShowRules,
    AddRule,
    Ok,
    RuleChanged(TreePath, u32, String)
}

pub enum Msg {
    TextChange(String),
    SearchPressed,
    ClearSearchPressed,
    RuleMsg(RuleMsg),
    ToggleAutoScroll(bool)
}

pub struct WorkbenchState {
    search_text: String,
    rules: RuleList,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            search_text: String::new(),
            rules: RuleList::new()
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
                Msg::RuleMsg(msg) => {
                    match msg {
                        RuleMsg::ShowRules => {
                            let state = state.borrow_mut();
                            let dlg = RulesDialog::new(&state.rules, tx.clone());
                            dlg.show();
                        }
                        RuleMsg::AddRule => {
                            let state = state.borrow_mut();
                            state.rules.add_rule(&CustomRule::new("New rule"));
                        }
                        RuleMsg::RuleChanged(path, column, value) => {
                            let state = state.borrow_mut();
                            state.rules.update(path, column, value);
                        }
                        RuleMsg::Ok => {
                            let state = state.borrow();
                            if let Ok(rules) = state.rules.get_rules() {
                                println!("Rules: {:?}", rules);
                                file_view.apply_rules(rules);
                            }
                        }
                    }
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