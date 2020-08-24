use gtk::prelude::*;

use gtk::{Orientation, WidgetExt, ContainerExt, ButtonExt, IconSize, ReliefStyle};
use crate::{RuleViewMsg, RuleListViewMsg};
use glib::bitflags::_core::cmp::Ordering;
use std::rc::Rc;
use uuid::Uuid;
use std::collections::HashMap;
use glib::bitflags::_core::cell::RefCell;
use gdk::RGBA;

#[derive(Debug, Default, Clone)]
pub struct Rule {
    pub id: uuid::Uuid,
    pub name: Option<String>,
    pub color: Option<String>,
    pub regex: Option<String>,
    pub is_system: bool,
}

impl Rule {
    pub fn new() -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: None,
            color: None,
            regex: None,
            is_system: false,
        }
    }
}

pub struct RuleView {
    container: gtk::Box,
    data: Rule,
    regex_txt: gtk::Entry,
}

impl RuleView {
    fn new<T: 'static + Clone + Fn(RuleViewMsg)>(rule: Rule, tx: T) -> Self {
        let default = String::from("New Rule");
        let name = rule.name.as_ref().unwrap_or(&default);

        let container = gtk::Box::new(Orientation::Horizontal, 4);

        let name_txt = gtk::Entry::new(); {
            let tx = tx.clone();
            name_txt.connect_changed(move |e| {
                let s = e.get_text().to_string();
                tx(RuleViewMsg::NameChanged(s));
            });
            name_txt.set_text(name);
            name_txt.set_sensitive(!rule.is_system);
            container.add(&name_txt);
        }

        let regex = gtk::Entry::new(); {
            let tx = tx.clone();
            regex.connect_changed(move |e| {
                let s = e.get_text().to_string();
                tx(RuleViewMsg::RegexChanged(s));
            });
            container.add(&regex);
            regex.set_sensitive(!rule.is_system);
            if let Some(r) = &rule.regex {
                regex.set_text(r);
            }
        }

        let color_btn = gtk::ColorButton::new(); {
            let tx = tx.clone();
            color_btn.connect_color_set(move |a|{
                let color = a.get_rgba();
                tx(RuleViewMsg::ColorChanged(color.to_string()));
            });

            container.add(&color_btn);
            if let Some(color) = &rule.color {
                let rgba = color.parse::<RGBA>().unwrap();
                color_btn.set_rgba(&rgba);
            }
        }

        let btn = gtk::Button::from_icon_name(Some("edit-delete-symbolic"), IconSize::Button); {
            btn.set_relief(ReliefStyle::None);
            let tx = tx.clone();
            btn.connect_clicked(move |_| {
                tx(RuleViewMsg::DeleteRule);
            });
            btn.set_sensitive(!rule.is_system);
            container.add(&btn);
        }

        Self {
            container,
            regex_txt: regex,
            data: rule.clone()
        }
    }

    pub fn update(&mut self, msg: RuleViewMsg) {
        match msg {
            RuleViewMsg::RegexChanged(regex) => {
                self.set_regex(Some(regex));
            }
            RuleViewMsg::NameChanged(name) => {
                self.data.name = Some(name);
            }
            RuleViewMsg::ColorChanged(color) => {
                self.data.color = Some(color);
            }
            RuleViewMsg::DeleteRule => {
                // Msg is handled in list view
            }
        }
    }

    pub fn set_regex(&mut self, regex: Option<String>) {
        if let Some(regex) = &regex {
            self.regex_txt.set_text(regex.as_str());
        }else {
            self.regex_txt.set_text("");
        }
        self.data.regex = regex;
    }

    fn view(&self) -> &gtk::Box {
        &self.container
    }
}

impl PartialEq for Rule {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl PartialOrd for Rule {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

#[derive(Clone)]
pub struct RuleList {
    rules: HashMap<Uuid, Rule>
}

pub const SEARCH_ID: &'static str = "ba5b70bb-57b9-4f5c-95c9-e80953ae113e";


pub struct RuleListView {
    container: gtk::Box,
    rules: HashMap<Uuid, RuleView>,
    rule_list: Rc<gtk::Box>,
    rule_view_id_map: Rc<RefCell<HashMap<Uuid, gtk::Box>>>,
    tx: Rc<dyn 'static + Fn(RuleListViewMsg)>
}

impl RuleListView {
    pub fn new<T>(tx: T) -> Self
        where T: 'static + Clone + Fn(RuleListViewMsg)
    {
        let rule_list = Rc::new(gtk::Box::new(Orientation::Vertical, 4));
        let toolbar = gtk::Box::new(Orientation::Horizontal, 0);
        toolbar.set_margin_bottom(4);
        let rule_view_id_map = Rc::new(RefCell::new(HashMap::new()));


        let add_btn = gtk::Button::from_icon_name(Some("list-add-symbolic"), IconSize::Button); {
            add_btn.set_relief(ReliefStyle::None);
            let tx = tx.clone();
            add_btn.connect_clicked(move |_| {
                let rule_data = Rule::new();
                tx(RuleListViewMsg::AddRule(rule_data));
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
            rules: HashMap::new(),
            tx: Rc::new(tx.clone())
        }
    }

    pub fn add_rules(&mut self, data: Vec<Rule>) {
        for rule in data {
            self.add_rule(rule);
        }
    }

    pub fn add_rule(&mut self, data: Rule) {
        let tx = self.tx.clone();
        let wrapper = gtk::Box::new(Orientation::Horizontal, 0);
        self.rule_list.add(&wrapper);
        let id = data.id.clone();
        let view = RuleView::new(data, move |msg| {
            (*tx)(RuleListViewMsg::RuleViewMsg(id.clone(), msg));
        });

        wrapper.add(view.view());
        self.rules.insert(id.clone(), view);
        self.rule_view_id_map.borrow_mut().insert(id, wrapper);
        self.rule_list.show_all();
    }

    pub fn get_rules(&self) -> Vec<Rule> {
        self.rules.values().map(|v|v.data.clone()).collect()
    }

    pub fn get_rule_by_id(&mut self, id: &str) -> Option<&mut RuleView> {
        let id = Uuid::parse_str(id).unwrap();
        self.rules.get_mut(&id)
    }

    pub fn update(&mut self, msg: RuleListViewMsg) {
        match msg {
            RuleListViewMsg::AddRule(rule) => {
                self.add_rule(rule.clone());
            }
            RuleListViewMsg::RuleViewMsg(id, msg) => {
                match msg {
                    RuleViewMsg::DeleteRule => {
                        self.delete(id);
                    }
                    _ => {
                        if let Some(rule_view) = self.rules.get_mut(&id) {
                            rule_view.update(msg);
                        }
                    }
                }
            }
        }
    }

    pub fn delete(&mut self, id: Uuid) {
        let map = self.rule_view_id_map.borrow_mut();
        if let Some(existing) = map.get(&id) {
            self.rule_list.remove(existing);
        }
        self.rules.remove(&id);
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}