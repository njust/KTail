use gtk::{Orientation, GtkWindowExt, WindowPosition, HeaderBar, WidgetExt, HeaderBarExt, DialogExt, ContainerExt, Label, TreeViewExt, ButtonExt, TreeModelExt};
use crate::file_view::util::create_col;
use gtk::prelude::GtkListStoreExtManual;
use glib::Sender;
use crate::file_view::workbench::{Msg, RuleMsg};

pub struct Rule {
    name: String,
    color: String,
    regex: String,
}

impl Rule {
    pub fn new(name: &str) -> Self {
        Self {
            name: String::from(name),
            color: String::new(),
            regex: String::new(),
        }
    }
}

pub struct RuleList {
    list_model: gtk::ListStore,
}

impl RuleList {
    pub fn new() -> Self {
        let list_model = gtk::ListStore::new(&[glib::Type::String, glib::Type::String, glib::Type::String]);
        list_model.connect_row_changed(|r,path,iter| {
           println!("Row changed!");
        });
        Self {
            list_model
        }
    }

    pub fn add_rule(&self, rule: &Rule) {
        self.list_model.insert_with_values(None, &[0,1,2], &[&rule.name, &rule.regex, &rule.color]);
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
            add_btn.connect_clicked(move |_| {
                tx.send(Msg::RuleMsg(RuleMsg::AddRule));
            });
            toolbar.add(&add_btn);
        }

        let list_view = gtk::TreeView::with_model(&rules.list_model);

        list_view.append_column(&create_col("Name", 0, |a,b,c| {
            println!("Cell changed")
        }));

        list_view.append_column(&create_col("Regex", 1, |a,b,c| {
            println!("Cell changed")
        }));

        list_view.append_column(&create_col("Color", 2, |a,b,c| {
            println!("Cell changed")
        }));

        container.add(&toolbar);
        container.add(&list_view);
        Self {
            container,
            list: rules
        }
    }

    fn add_rule(&self, rule: &Rule) {
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

    fn add_rule(&mut self, rule: &Rule) {
        self.list.add_rule(rule);
    }

    pub fn show(&self) {
        self.dlg.show_all();
    }
}