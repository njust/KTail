use gtk::prelude::*;

use gtk::{Orientation, WidgetExt, ContainerExt, ButtonExt, IconSize, ReliefStyle};
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
use crate::Result;
use std::rc::Rc;

#[derive(Debug, Default, Clone)]
pub struct Highlighter {
    pub id: uuid::Uuid,
    pub name: Option<String>,
    pub color: Option<String>,
    pub regex: Option<String>,
    pub is_system: bool,
    pub rule_type: String,
}

impl Highlighter {
    pub fn new(rule_type: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: None,
            color: None,
            regex: None,
            is_system: false,
            rule_type: rule_type.to_string(),
        }
    }

    pub fn is_exclude(&self) -> bool {
        self.rule_type == RULE_TYPE_EXCLUDE
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
pub const ID_PROP: &'static str = "id";
pub const NAME_PROP: &'static str = "name";
pub const REGEX_PROP: &'static str = "regex";
pub const COLOR_PROP: &'static str = "color";
pub const IS_SYSTEM_PROP: &'static str = "isSystem";
pub const RULE_TYPE: &'static str = "ruleType";

pub const RULE_TYPE_HIGHLIGHT: &'static str = "highlight";
pub const RULE_TYPE_EXCLUDE: &'static str = "exclude";


pub struct HighlighterListView {
    container: gtk::Box,
    highlighter_list_data: Rc<gio::ListStore>
}

data_model!(HighlighterData);
impl DataModelDescription for HighlighterData {
    const NAME: &'static str = "HighlighterData";
    fn get_properties() -> &'static [Property<'static>] {
        &[
            subclass::Property(ID_PROP, |name| {
                glib::ParamSpec::string(name,"Id","Id",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property(NAME_PROP, |name| {
                glib::ParamSpec::string(name,"Name","Name",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property(REGEX_PROP, |name| {
                glib::ParamSpec::string(name,"Regex","Regex",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property(COLOR_PROP, |name| {
                glib::ParamSpec::string(name,"Color","Color",None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property(IS_SYSTEM_PROP, |name| {
                glib::ParamSpec::boolean(name,"System","System",false, glib::ParamFlags::READWRITE)
            }),
            subclass::Property(RULE_TYPE, |name|{
                glib::ParamSpec::string(name, "Type", "Type", None, glib::ParamFlags::READWRITE)
            }),
        ]
    }
}

impl HighlighterListView {
    pub fn new<>() -> Self {
        let list = Rc::new(gio::ListStore::new(HighlighterData::static_type()));
        let list_box = gtk::ListBox::new();
        {
            let list = list.clone();
            list_box.bind_model(Some(&*list.clone()), move |item| {
                let row = gtk::ListBoxRow::new();
                let container = gtk::Box::new(Orientation::Horizontal, 4);
                let item = item.downcast_ref::<HighlighterData>().expect("Row data is of wrong type");

                let id = item.get_property(ID_PROP).ok()
                    .and_then(|id| id.get::<String>().ok())
                    .and_then(|id| id).unwrap();

                let is_system = item.get_property(IS_SYSTEM_PROP).ok()
                    .and_then(|id| id.get::<bool>().ok())
                    .and_then(|id| id).unwrap();

                let type_selector = gtk::ComboBoxTextBuilder::new()
                    .sensitive(!is_system)
                    .build();

                type_selector.append(Some(RULE_TYPE_HIGHLIGHT), "Highlight");
                type_selector.append(Some(RULE_TYPE_EXCLUDE), "Exclude");
                // type_selector.append(Some("include"), "Include");
                item.bind_property(RULE_TYPE, &type_selector, "active-id")
                    .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();

                container.add(&type_selector);

                let name_entry = gtk::Entry::new();
                item.bind_property(NAME_PROP, &name_entry, "text")
                    .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
                container.add(&name_entry);

                let regex_entry = gtk::Entry::new();
                item.bind_property(REGEX_PROP, &regex_entry, "text")
                    .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
                container.add(&regex_entry);

                let color_button = gtk::ColorButton::new();
                item.bind_property(COLOR_PROP, &color_button, "rgba")
                    .transform_to(|_, value| {
                        let rgba =
                            value.get::<String>().ok()
                                .and_then(|c| c)
                                .and_then(|c| c.parse::<RGBA>().ok())
                                .unwrap_or(RGBA::black());

                        Some(glib::Value::from(Some(&rgba)))
                    })
                    .transform_from(|_, value| {
                        let data = value.get::<RGBA>().unwrap().unwrap();
                        Some(glib::Value::from(&data.to_string()))
                    })
                    .flags(glib::BindingFlags::DEFAULT | glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
                container.add(&color_button);

                let btn = gtk::ButtonBuilder::new()
                    .image(&gtk::Image::from_icon_name(Some("edit-delete-symbolic"), IconSize::Button))
                    .sensitive(!is_system)
                    .relief(ReliefStyle::None)
                    .build();
                {
                    let list = list.clone();
                    btn.connect_clicked(move |_| {
                        if let Some(idx) = Self::get_highlighter_idx(&id.to_string(), list.clone()) {
                            list.remove(idx);
                        }
                    });
                    container.add(&btn);
                }

                row.add(&container);
                row.show_all();
                row.upcast::<gtk::Widget>()
            });
        }

        let toolbar = gtk::Box::new(Orientation::Horizontal, 0);
        toolbar.set_margin_bottom(4);

        let add_btn = gtk::ButtonBuilder::new()
            .label("Add Rule")
            .image(&gtk::Image::from_icon_name(Some("list-add-symbolic"), IconSize::Button))
            .relief(ReliefStyle::None)
            .always_show_image(true)
            .build();
        {
            let list = list.clone();
            add_btn.connect_clicked(move |_| {
                let rule_data = Highlighter::new(RULE_TYPE_HIGHLIGHT);
                Self::add(rule_data, list.clone());
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

    pub fn add_highlighters(&self, data: Vec<Highlighter>) {
        for rule in data {
            self.add_highlighter(rule);
        }
    }

    pub fn add_highlighter(&self, data: Highlighter) {
        Self::add(data, self.highlighter_list_data.clone());
    }

    fn add(data: Highlighter, highlighter_list_data: Rc<gio::ListStore>) {
        highlighter_list_data.append(&HighlighterData::new(&[
            (ID_PROP, &data.id.to_string()),
            (NAME_PROP, &data.name.unwrap_or_default()),
            (REGEX_PROP, &data.regex.unwrap_or_default()),
            (COLOR_PROP, &data.color),
            (IS_SYSTEM_PROP, &data.is_system),
            (RULE_TYPE, &data.rule_type)
        ]));
    }


    pub fn get_highlighter(&self) -> Result<Vec<Highlighter>> {
        let cnt = self.highlighter_list_data.get_n_items();
        let mut rules = vec![];
        for i in 0..cnt {
            if let Some(o) = self.highlighter_list_data.get_object(i) {
                let id = o.get_property(ID_PROP)?.get::<String>()?.ok_or("No id")?;
                let name = o.get_property(NAME_PROP)?.get::<String>()?.and_then(|s|if s.len() <= 0 {None}else {Some(s)});
                let regex = o.get_property(REGEX_PROP)?.get::<String>()?.and_then(|s|if s.len() <= 0 {None}else {Some(s)});
                let color = o.get_property(COLOR_PROP)?.get::<String>().unwrap_or(None);
                let rule_type = o.get_property(RULE_TYPE)?.get::<String>()?.ok_or("No type for rule")?;
                let is_system = o.get_property(IS_SYSTEM_PROP)?.get::<bool>()?.unwrap_or(false);
                rules.push(Highlighter {
                    id: Uuid::parse_str(&id).unwrap(),
                    name,
                    regex,
                    color,
                    is_system,
                    rule_type
                })
            }
        }
        Ok(rules)
    }

    pub fn set_regex(&mut self, id: &str, regex: &String) {
        if let Some(o) = Self::get_highlighter_by_id(id, self.highlighter_list_data.clone()) {
            if let Err(e) = o.set_property(REGEX_PROP, &regex) {
                error!("Could not set regex: {}", e);
            }
        }
    }

    fn get_highlighter_by_id(rid: &str, highlighter_list_data: Rc<gio::ListStore>) -> Option<Object> {
        Self::get_highlighter_idx(rid, highlighter_list_data.clone()).and_then(|idx|highlighter_list_data.get_object(idx))
    }

    fn get_highlighter_idx(sid: &str, highlighter_list_data: Rc<gio::ListStore>) -> Option<u32> {
        let cnt = highlighter_list_data.get_n_items();
        for i in 0..cnt {
            if let Some(o) = highlighter_list_data.get_object(i) {
                let id = o.get_property(ID_PROP).unwrap().get::<String>().unwrap().unwrap();
                if id == sid {
                    return Some(i)
                }
            }
        }
        None
    }

    pub fn view(&self) -> &gtk::Box {
        &self.container
    }
}