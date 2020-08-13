use gtk::prelude::*;

use gtk::{Orientation, GtkWindowExt, WindowPosition, HeaderBar, WidgetExt, HeaderBarExt, DialogExt, ContainerExt, ButtonExt};
use glib::{Sender};
use crate::file_view::workbench::{Msg, RuleMsg};
use glib::bitflags::_core::cmp::Ordering;
use crate::file_view::SEARCH_TAG;
use std::rc::Rc;
use uuid::Uuid;
use std::collections::HashMap;
use glib::bitflags::_core::cell::RefCell;


#[derive(Debug, Clone)]
pub enum Rule {
    UserSearch(String),
    CustomRule(CustomRule),
}

impl Rule {
    pub fn get_regex(&self) -> Option<&String> {
        match self {
            Rule::UserSearch(s) => Some(s),
            Rule::CustomRule(rule) => rule.regex.as_ref()
        }
    }

    pub fn get_id(&self) -> String {
        match self {
            Rule::UserSearch(s) => s.clone(),
            Rule::CustomRule(rule) => rule.id.to_string()
        }
    }

    pub fn get_tag(&self) -> String {
        match self {
            Rule::UserSearch(_) => String::from(SEARCH_TAG),
            Rule::CustomRule(rule) => rule.id.to_string()
        }
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Rule::UserSearch(s) => {
                if let Rule::UserSearch(s2) = other {
                    return s == s2;
                }
            }
            Rule::CustomRule(rule) => {
                if let Rule::CustomRule(rule2) = other {
                    return rule.regex == rule2.regex && rule.name == rule2.name;
                }
            }
        }

        false
    }
}

#[derive(Debug, Default, Clone)]
pub struct CustomRule {
    pub id: uuid::Uuid,
    pub name: Option<String>,
    pub color: Option<String>,
    pub regex: Option<String>,
}

impl CustomRule {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: None,
            color: None,
            regex: None,
        }
    }
}

struct CustomRuleView {
    container: gtk::Box,
}

impl CustomRuleView {
    fn new(rule: &CustomRule, tx: Sender<Msg>) -> Self {
        let id = rule.id;
        let default = String::from("New Rule");
        let name = rule.name.as_ref().unwrap_or(&default);

        let container = gtk::Box::new(Orientation::Horizontal, 20);

        let name_txt = gtk::Entry::new(); {
            let tx = tx.clone();
            name_txt.connect_changed(move |e| {
                let s = e.get_text().to_string();
                tx.send(Msg::RuleMsg(RuleMsg::NameChanged(id, s))).expect("Could not send name change");
            });
            name_txt.set_text(name);
            container.add(&name_txt);
        }

        let regex = gtk::Entry::new(); {
            let tx = tx.clone();
            regex.connect_changed(move |e| {
                let s = e.get_text().to_string();
                tx.send(Msg::RuleMsg(RuleMsg::RegexChanged(id, s))).expect("Could not send regex changed");
            });
            container.add(&regex);
        }

        let color_btn = gtk::ColorButton::new(); {
            let tx = tx.clone();
            color_btn.connect_color_set(move |a|{
                let color = a.get_rgba();
                tx.send(Msg::RuleMsg(RuleMsg::ColorChanged(id, color.to_string()))).expect("Could not send color change");
            });

            container.add(&color_btn);
        }

        let btn = gtk::Button::with_label("Delete"); {
            let tx = tx.clone();
            btn.connect_clicked(move |_| {
                tx.send(Msg::RuleMsg(RuleMsg::DeleteRule(id))).expect("Could not send delete rule");
            });
            container.add(&btn);
        }

        Self {
            container
        }
    }

    fn view(&self) -> &gtk::Box {
        &self.container
    }
}

impl PartialEq for CustomRule {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl PartialOrd for CustomRule {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

#[derive(Clone)]
pub struct RuleList {
    rules: HashMap<Uuid, CustomRule>
}

impl RuleList {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new()
        }
    }
    pub fn color_changed(&mut self, id: Uuid, color: String) {
        if let Some(rule) = self.rules.get_mut(&id) {
            rule.color = Some(color)
        }
    }

    pub fn name_changed(&mut self, id: Uuid, name: String) {
        if let Some(rule) = self.rules.get_mut(&id) {
            rule.name = Some(name)
        }
    }

    pub fn regex_changed(&mut self, id: Uuid, regex: String) {
        if let Some(rule) = self.rules.get_mut(&id) {
            rule.regex = Some(regex)
        }
    }

    pub fn add_rule(&mut self, rule: CustomRule) {
        self.rules.insert(rule.id, rule);
    }

    pub fn get_rules(&self) -> Vec<CustomRule> {
        self.rules.values().map(|e|e.clone()).collect()
    }

    pub fn delete(&mut self, id: Uuid) {
        self.rules.remove(&id);
    }

}

pub struct RuleListView {
    container: gtk::Box,
    rules: RuleList,
    rule_list: Rc<gtk::Box>,
    rule_view_id_map: Rc<RefCell<HashMap<Uuid, gtk::Box>>>
}

impl RuleListView {
    pub fn new(tx: Sender<Msg>) -> Self {
        let rule_list = Rc::new(gtk::Box::new(Orientation::Vertical, 0));
        let toolbar = gtk::Box::new(Orientation::Horizontal, 0);
        let rule_view_id_map = Rc::new(RefCell::new(HashMap::new()));

        let add_btn = gtk::Button::with_label("Add"); {
            let tx = tx.clone();
            let rule_list = rule_list.clone();
            let rule_id_view_map = rule_view_id_map.clone();
            add_btn.connect_clicked(move |_| {
                let rule_data = CustomRule::new();
                let rule_view = CustomRuleView::new(&rule_data, tx.clone());

                let wrapper = gtk::Box::new(Orientation::Horizontal, 0);
                wrapper.add(rule_view.view());

                rule_list.add(&wrapper);
                rule_id_view_map.borrow_mut().insert(rule_data.id, wrapper);
                rule_list.show_all();
                tx.send(Msg::RuleMsg(RuleMsg::AddRule(rule_data))).expect("Could not send add rule");
            });
            toolbar.add(&add_btn);
        }

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(&toolbar);
        container.add(&*rule_list);

        Self {
            container,
            rule_list,
            rule_view_id_map,
            rules: RuleList::new()
        }
    }

    pub fn get_rules(&self) -> Vec<CustomRule> {
        self.rules.get_rules()
    }

    pub fn update(&mut self, msg: RuleMsg) {
        match msg {
            RuleMsg::AddRule(rule) => {
                self.rules.add_rule(rule);
            }
            RuleMsg::NameChanged(id, name) => {
                self.rules.name_changed(id, name);
            }
            RuleMsg::RegexChanged(id, regex) => {
                self.rules.regex_changed(id, regex);
            }
            RuleMsg::ColorChanged(id, color) => {
                self.rules.color_changed(id, color);
            }
            RuleMsg::DeleteRule(id) => {
                self.rules.delete(id);
                self.delete(id);
            }
        }
    }

    pub fn delete(&mut self, id: Uuid) {
        let map = self.rule_view_id_map.borrow_mut();
        if let Some(existing) = map.get(&id) {
            self.rule_list.remove(existing);
        }
    }

    fn view(&self) -> &gtk::Box {
        &self.container
    }
}

pub struct RulesDialog {
    dlg: gtk::Dialog,
}

impl RulesDialog {
    pub fn new(rule_list_view: &RuleListView, tx: Sender<Msg>) -> Self {
        let dlg = gtk::Dialog::new();
        dlg.set_position(WindowPosition::Mouse);
        dlg.set_default_size(400, 200);
        let header_bar = HeaderBar::new();
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Rules"));
        dlg.set_titlebar(Some(&header_bar));
        dlg.set_modal(true);

        let content = dlg.get_content_area();
        content.add(rule_list_view.view());

        dlg.connect_delete_event(move |dlg, _| {
            tx.send(Msg::ApplyRules).expect("Could not send apply rules");
            dlg.hide();
            gtk::Inhibit(true)
        });
        Self {
            dlg,
        }
    }

    pub fn show(&self) {
        self.dlg.show_all();
    }
}