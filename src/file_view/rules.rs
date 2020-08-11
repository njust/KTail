use gtk::{Orientation, GtkWindowExt, WindowPosition, HeaderBar, WidgetExt, HeaderBarExt, DialogExt, ContainerExt, Label, TreeViewExt, ButtonExt, TreeModelExt, CellRendererText, TreePath, TreeIter};
use crate::file_view::util::create_col;
use gtk::prelude::GtkListStoreExtManual;
use glib::Sender;
use crate::file_view::workbench::{Msg, RuleMsg};
use std::error::Error;
use glib::bitflags::_core::cmp::Ordering;
use crate::file_view::SEARCH_TAG;


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

    pub fn get_tag(&self) -> String {
        match self {
            Rule::UserSearch(s) => String::from(SEARCH_TAG),
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
    pub fn new(name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: Some(String::from(name)),
            color: None,
            regex: None,
        }
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

pub struct RuleList {
    list_model: gtk::ListStore,
}

impl RuleList {
    pub fn new() -> Self {
        let list_model = gtk::ListStore::new(&[glib::Type::String, glib::Type::String, glib::Type::String, glib::Type::String]);
        list_model.connect_row_changed(|r,path,iter| {
           println!("Row changed!");
        });
        Self {
            list_model
        }
    }

    pub fn update(&self, path: TreePath, column: u32, data: String) {
        if let Some(item) = self.list_model.get_iter(&path) {
            self.list_model.set(&item, &[column], &[&data]);
        }
    }

    pub fn add_rule(&self, rule: &CustomRule) {
        self.list_model.insert_with_values(None, &[0,1,2,3], &[&rule.id.to_string(), &rule.name, &rule.regex, &rule.color]);
    }

    pub fn get_rules(&self) -> Result<Vec<CustomRule>, Box<dyn Error>> {
        let mut rules = vec![];
        if let Some(iter) = self.list_model.get_iter_first() {
            let rule = self.get_rule_for_iter(&iter)?;
            rules.push(rule);
            while self.list_model.iter_next(&iter) {
                let rule = self.get_rule_for_iter(&iter)?;
                rules.push(rule);
            }
        }
        Ok(rules)
    }

    pub fn get_rule_for_iter(&self, iter: &TreeIter) -> Result<CustomRule, Box<dyn Error>> {
        let id = self.list_model.get_value(&iter, 0).get::<String>()?.ok_or("Rule without id!")?;
        let name = self.list_model.get_value(&iter, 1).get::<String>()?;
        let regex = self.list_model.get_value(&iter, 2).get::<String>()?;
        let color = self.list_model.get_value(&iter, 3).get::<String>()?;

        let id = uuid::Uuid::parse_str(&id)?;
        Ok(CustomRule {
            id, name, regex, color
        })
    }

    pub fn model(&self) -> &gtk::ListStore {
        &self.list_model
    }
}

pub struct RuleListView<'a> {
    container: gtk::Box,
    list: &'a RuleList,
}

impl<'a> RuleListView<'a> {
    pub fn new(rules: &'a RuleList, tx: Sender<Msg>) -> Self {
        let container = gtk::Box::new(Orientation::Vertical, 0);
        let toolbar = gtk::Box::new(Orientation::Horizontal, 0);
        let add_btn = gtk::Button::with_label("Add"); {
            let tx = tx.clone();
            add_btn.connect_clicked(move |_| {
                tx.send(Msg::RuleMsg(RuleMsg::AddRule));
            });
            toolbar.add(&add_btn);
        }

        let ok_btn = gtk::Button::with_label("Ok"); {
            let tx = tx.clone();
            ok_btn.connect_clicked(move |_| {
                tx.send(Msg::RuleMsg(RuleMsg::Ok));
            });
            toolbar.add(&ok_btn);
        }

        let list_view = gtk::TreeView::with_model(&rules.list_model);

        list_view.append_column(&create_col("Name", 1, tx.clone()));
        list_view.append_column(&create_col("Regex", 2, tx.clone()));
        list_view.append_column(&create_col("Color", 3, tx.clone()));

        container.add(&toolbar);
        container.add(&list_view);
        Self {
            container,
            list: rules
        }
    }

    fn add_rule(&self, rule: &CustomRule) {
        self.list.add_rule(rule)
    }

    fn view(&self) -> &gtk::Box {
        &self.container
    }
}

pub struct RulesDialog<'a> {
    dlg: gtk::Dialog,
    list: RuleListView<'a>,
}

impl<'a> RulesDialog<'a> {
    pub fn new(rules: &'a RuleList, tx: Sender<Msg>) -> Self {
        let dlg = gtk::Dialog::new();
        dlg.set_position(WindowPosition::Mouse);
        dlg.set_default_size(300, 200);
        let header_bar = HeaderBar::new();
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Rules"));
        dlg.set_titlebar(Some(&header_bar));
        dlg.set_modal(true);

        let rules = RuleListView::new(rules, tx);
        let content = dlg.get_content_area();
        content.add(rules.view());

        dlg.connect_delete_event(|dlg, _| {
            dlg.hide();
            gtk::Inhibit(true)
        });
        Self {
            dlg,
            list: rules
        }
    }

    fn add_rule(&mut self, rule: &CustomRule) {
        self.list.add_rule(rule);
    }

    pub fn show(&self) {
        self.dlg.show_all();
    }
}