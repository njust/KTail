use gtk::prelude::*;

use gtk::{Orientation, WidgetExt, ContainerExt, ButtonExt, IconSize, ReliefStyle};
use crate::model::{RuleListViewMsg};
use glib::bitflags::_core::cmp::Ordering;
use uuid::Uuid;
use std::collections::HashMap;
use gdk::RGBA;
use log::{error};

use glib_data_model_helper::{
    prelude::*,
    data_model,
};
use gio::{ListStoreExt, ListModelExt};
use glib::Object;

#[derive(Debug, Default, Clone)]
pub struct Highlighter {
    pub id: uuid::Uuid,
    pub name: Option<String>,
    pub color: Option<String>,
    pub regex: Option<String>,
    pub is_system: bool,
}

impl Highlighter {
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

impl PartialEq for Highlighter {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl PartialOrd for Highlighter {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

#[derive(Clone)]
pub struct HighlighterList {
    rules: HashMap<Uuid, Highlighter>
}

pub const SEARCH_ID: &'static str = "ba5b70bb-57b9-4f5c-95c9-e80953ae113e";


pub struct HighlighterListView {
    container: gtk::Box,
    highlighter_list_data: gio::ListStore
}

data_model!(HighlighterData);
impl DataModelDescription for HighlighterData {
    const NAME: &'static str = "HighlighterData";
    fn get_properties() -> &'static [Property<'static>] {
        &[
            subclass::Property("id", |name| {
                glib::ParamSpec::string(name,"Id","Id",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property("name", |name| {
                glib::ParamSpec::string(name,"Name","Name",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property("regex", |name| {
                glib::ParamSpec::string(name,"Regex","Regex",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property("color", |name| {
                glib::ParamSpec::string(name,"Color","Color",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property("isSystem", |name| {
                glib::ParamSpec::boolean(name,"System","System",false, glib::ParamFlags::READWRITE)
            })
        ]
    }
}

impl HighlighterListView {
    pub fn new<T>(tx: T) -> Self
        where T: 'static + Clone + Fn(RuleListViewMsg)
    {
        let list = gio::ListStore::new(HighlighterData::static_type());
        let list_box = gtk::ListBox::new();
        let tx2 = tx.clone();
        list_box.bind_model(Some(&list), move |item| {
            let row = gtk::ListBoxRow::new();
            let container = gtk::Box::new(Orientation::Horizontal, 4);
            let item = item.downcast_ref::<HighlighterData>().expect("Row data is of wrong type");

            let id = item.get_property("id").ok()
                .and_then(|id| id.get::<String>().ok())
                .and_then(|id|id).unwrap();

            let is_system = item.get_property("isSystem").ok()
                .and_then(|id| id.get::<bool>().ok())
                .and_then(|id|id).unwrap();

            let name_entry = gtk::Entry::new();
            item.bind_property("name", &name_entry, "text")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
            container.add(&name_entry);

            let regex_entry = gtk::Entry::new();
            item.bind_property("regex", &regex_entry, "text")
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
            container.add(&regex_entry);

            let color_button = gtk::ColorButton::new();
            item.bind_property("color", &color_button, "rgba")
                .transform_to(|_, value| {
                    let rgba =
                        value.get::<String>().ok()
                            .and_then(|c|c)
                            .and_then(|c|c.parse::<RGBA>().ok())
                            .unwrap_or(RGBA::black());

                    Some(glib::Value::from(Some(&rgba)))
                })
                .transform_from(|_,value| {
                    let data = value.get::<RGBA>().unwrap().unwrap();
                    Some(glib::Value::from(&data.to_string()))
                })
                .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
            container.add(&color_button);

            let btn = gtk::Button::from_icon_name(Some("edit-delete-symbolic"), IconSize::Button); {
                btn.set_relief(ReliefStyle::None);
                let tx = tx2.clone();
                btn.connect_clicked(move |_| {
                    tx(RuleListViewMsg::DeleteRule(id.clone()));
                });
                btn.set_sensitive(!is_system);
                container.add(&btn);
            }

            row.add(&container);
            row.show_all();
            row.upcast::<gtk::Widget>()
        });

        let toolbar = gtk::Box::new(Orientation::Horizontal, 0);
        toolbar.set_margin_bottom(4);

        let add_btn = gtk::Button::from_icon_name(Some("list-add-symbolic"), IconSize::Button); {
            add_btn.set_relief(ReliefStyle::None);
            let tx = tx.clone();
            add_btn.connect_clicked(move |_| {
                let rule_data = Highlighter::new();
                tx(RuleListViewMsg::AddRule(rule_data));
            });
            toolbar.add(&add_btn);
        }

        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.add(&toolbar);
        container.add(&list_box);

        Self {
            container,
            highlighter_list_data: list
        }
    }

    pub fn add_highlighters(&mut self, data: Vec<Highlighter>) {
        for rule in data {
            self.add_highlighter(rule);
        }
    }

    pub fn add_highlighter(&mut self, data: Highlighter) {
        let id = data.id.to_string();
        let name = data.name.unwrap_or(String::new());
        let regex = data.regex.unwrap_or(String::new());
        let color = data.color.unwrap_or(String::new());
        self.highlighter_list_data.append(&HighlighterData::new(&[("id", &id), ("name", &name), ("regex", &regex), ("color", &color), ("isSystem", &data.is_system)]));
    }

    pub fn get_highlighter(&self) -> Vec<Highlighter> {
        let cnt = self.highlighter_list_data.get_n_items();
        let mut rules = vec![];
        for i in 0..cnt {
            if let Some(o) = self.highlighter_list_data.get_object(i) {
                let id = o.get_property("id").unwrap().get::<String>().unwrap().unwrap();
                let name = o.get_property("name").unwrap().get::<String>().unwrap().and_then(|s|if s.len() <= 0 {None}else {Some(s)});
                let regex = o.get_property("regex").unwrap().get::<String>().unwrap().and_then(|s|if s.len() <= 0 {None}else {Some(s)});
                let color = o.get_property("color").unwrap().get::<String>().unwrap();
                let is_system = o.get_property("isSystem").unwrap().get::<bool>().unwrap().unwrap_or(false);
                rules.push(Highlighter {
                    id: Uuid::parse_str(&id).unwrap(),
                    name,
                    regex,
                    color,
                    is_system
                })
            }
        }
        rules
    }

    pub fn set_regex(&mut self, id: &str, regex: &String) {
        if let Some(o) = self.get_highlighter_by_id(id) {
            if let Err(e) = o.set_property("regex", &regex) {
                error!("Could not set regex: {}", e);
            }
        }
    }

    fn get_highlighter_by_id(&self, rid: &str) -> Option<Object> {
        self.get_highlighter_idx(rid).and_then(|idx|self.highlighter_list_data.get_object(idx))
    }

    fn get_highlighter_idx(&self, sid: &str) -> Option<u32> {
        let cnt = self.highlighter_list_data.get_n_items();
        for i in 0..cnt {
            if let Some(o) = self.highlighter_list_data.get_object(i) {
                let id = o.get_property("id").unwrap().get::<String>().unwrap().unwrap();
                if id == sid {
                    return Some(i)
                }
            }
        }
        None
    }

    pub fn update(&mut self, msg: RuleListViewMsg) {
        match msg {
            RuleListViewMsg::AddRule(rule) => {
                self.add_highlighter(rule.clone());
            }
            RuleListViewMsg::DeleteRule(id) => {
                if let Some(idx) = self.get_highlighter_idx(&id.to_string()) {
                    self.highlighter_list_data.remove(idx);
                }
            }
        }
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}