use gtk4_helper::{
    gtk,
    glib,
    gio,
    model::prelude::*,
};
use gtk4_helper::gio::ListStore;

use gtk4_helper::gtk::{NONE_EXPRESSION, NONE_SORTER, ColumnView, Align, Sorter, SelectionModel, SortListModel};
use crate::gtk::PropertyExpression;

pub fn string_sorter(pe: &PropertyExpression) -> gtk::StringSorter {
    gtk::StringSorter::new(Some(&pe))
}

pub fn single_selection_model(sort_view: &SortListModel) -> gtk::SingleSelection {
    gtk::SingleSelection::new(Some(sort_view))
}

pub fn multi_selection_model(sort_view: &SortListModel) -> gtk::MultiSelection {
    gtk::MultiSelection::new(Some(sort_view))
}

pub fn create_column_view<P, T>(item_type: glib::types::Type, sel_model_factory: T) -> (ColumnView, ListStore)
    where P: IsA<SelectionModel>,
          T: Fn(&SortListModel) -> P
{
    let list_store = gio::ListStore::new(item_type);
    let sort_view = gtk::SortListModel::new(Some(&list_store), NONE_SORTER);

    let sel_model = sel_model_factory(&sort_view);
    let column_view = gtk::ColumnViewBuilder::new()
        .model(&sel_model)
        .build();

    if let Some(so) = column_view.sorter() {
        sort_view.set_sorter(Some(&so));
    }

    (column_view, list_store)
}

fn create_item_label(item: &gtk::ListItem, property: &str) {
    if let Some(obj) = item.item() {
        let lbl = gtk::LabelBuilder::new()
            .halign(Align::Start)
            .build();

        obj.bind_property(property, &lbl, "label")
            .flags(glib::BindingFlags::DEFAULT |glib::BindingFlags::SYNC_CREATE | glib::BindingFlags::BIDIRECTIONAL).build();
        item.set_child(Some(&lbl));
    }
}

pub fn create_column<P, T, I>(column_view: &ColumnView, ty: glib::Type, property: &'static str, title: &str, sorter_factory: T, item_factory: I)
    where P: IsA<Sorter>,
          T: Fn(&PropertyExpression) -> P,
          I: 'static + Fn(&gtk::ListItem, &str)
{
    let column_factory = gtk::SignalListItemFactory::new();
    column_factory.connect_bind(move |_, item| {
        item_factory(item, property)
    });

    let prop_exp = gtk::PropertyExpression::new(ty, NONE_EXPRESSION, property);
    let mut col_builder = gtk::ColumnViewColumnBuilder::new()
        .title(title)
        .factory(&column_factory);

    let sorter = sorter_factory(&prop_exp);
    col_builder = col_builder.sorter(&sorter);
    column_view.append_column(&col_builder.expand(true).build());
}

pub fn create_label_column<P, T>(column_view: &ColumnView, ty: glib::Type, property: &'static str, title: &str, sorter_factory: T)
    where P: IsA<Sorter>,
          T: Fn(&PropertyExpression) -> P
{
    create_column(column_view, ty, property, title, sorter_factory, create_item_label)
}

pub struct ButtonOptions {
    pub label: Option<&'static str>,
    pub image: Option<&'static str>,
}

pub fn create_button_column<I>(column_view: &ColumnView, title: &str, click_handler: I, opt: ButtonOptions)
    where I: 'static + Fn(u32) + Clone
{
    let column_factory = gtk::SignalListItemFactory::new();
    column_factory.connect_bind(move |_, item| {
        let click_handler = click_handler.clone();
        let pos = item.position();
        let mut btn = gtk::ButtonBuilder::new()
            .halign(Align::Center);

        if let Some(lbl) = opt.label {
            btn = btn.label(lbl);
        }

        if let Some(img) = opt.image {
            btn = btn.icon_name(img);
        }

        let btn = btn.build();
        btn.connect_clicked(move |_| {
            click_handler(pos);
        });
        item.set_child(Some(&btn));
    });

    let col_builder = gtk::ColumnViewColumnBuilder::new()
        .title(title)
        .factory(&column_factory);

    column_view.append_column(&col_builder.expand(true).build());
}