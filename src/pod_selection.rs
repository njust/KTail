use gtk::prelude::*;
use gtk::{ApplicationWindow, ResponseType, Orientation, TreeStore, WindowPosition, TreeIter, SortColumn, SortType, ScrolledWindow, DialogFlags, MessageType, ButtonsType, HeaderBarBuilder, BoxBuilder};
use std::rc::Rc;
use gio::{SimpleAction};
use log::{error};

use crate::util::{create_col, ColumnType};
use glib::Sender;
use k8s_client::{KubeConfig, KubeClient, Pod};
use crate::model::{Msg, LogViewData, CreateKubeLogData, CreateLogView};
use std::error::Error;
use std::collections::{HashSet};
use crate::highlighters::HighlighterListView;
use crate::get_default_highlighters;

const UNIT_MINUTES: &'static str = "UNIT_MINUTES";
const UNIT_HOURS: &'static str = "UNIT_HOURS";

fn map_model(model: &TreeStore, iter: &TreeIter) -> Option<(String, bool)> {
    let name = model.get_value(iter, 0).get::<String>().unwrap_or(None)?;
    let active = model.get_value(iter, 1).get::<bool>().unwrap_or(None)?;
    Some((name, active))
}

fn get_pods() -> Result<Vec<Pod>, Box<dyn Error>> {
    glib::MainContext::default().block_on(async move  {
        match KubeClient::load_conf(None) {
            Ok(c) => {
                c.pods().await
            }
            Err(e) => {
                Err(e)
            }
        }
    })
}

fn set_pods(pod_model: &TreeStore, pods: Vec<Pod>, include_replicas: bool) {
    pod_model.clear();
    let mut duplicates = HashSet::new();
    for pod in pods.into_iter() {
        let name = if include_replicas {
          pod.spec.containers.first().map(|c|c.name.clone()).unwrap_or_default()
        }else {
            pod.metadata.name.unwrap_or_default()
        };

        if !duplicates.contains(&name) {
            pod_model.insert_with_values(None, None, &[0, 1], &[&name, &false]);
            duplicates.insert(name);
        }
    }
}

pub fn create_open_kube_action(tx: Sender<Msg>) -> Option<SimpleAction> {
    if !KubeConfig::default_path().exists() {
        return None;
    }

    let kube_action = SimpleAction::new("kube", None);

    let header_bar = HeaderBarBuilder::new()
        .show_close_button(true)
        .title("Pods")
        .build();

    let dlg = gtk::DialogBuilder::new()
        .window_position(WindowPosition::CenterOnParent)
        .default_width(450)
        .default_height(600)
        .modal(true)
        .build();

    dlg.set_titlebar(Some(&header_bar));

    let service_model = Rc::new(TreeStore::new(&[glib::Type::String, glib::Type::Bool]));
    service_model.set_sort_column_id(SortColumn::Index(0), SortType::Ascending);
    let list = gtk::TreeView::with_model(&*service_model);

    list.append_column(&create_col(Some("Add"), 1, ColumnType::Bool, service_model.clone()));
    list.append_column(&create_col(Some("Pod"), 0, ColumnType::String, service_model.clone()));

    let content = dlg.get_content_area();
    let scroll_wnd = ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);
    scroll_wnd.set_property_expand(true);
    scroll_wnd.add(&list);
    content.add(&scroll_wnd);

    let log_separate_tab_checkbox = gtk::CheckButton::with_label("Logs in separate tab");
    log_separate_tab_checkbox.set_active(true);
    content.add(&log_separate_tab_checkbox);

    let include_replicas = gtk::CheckButton::with_label("Include replicas");
    {
        let service_model = service_model.clone();
        include_replicas.set_active(true);
        include_replicas.connect_toggled(move |checkbox| {
            if let Ok(pods) = get_pods() {
                set_pods(&service_model, pods, checkbox.get_active());
            }
        });
        content.add(&include_replicas);
    }

    let adjustment = gtk::Adjustment::new(4.0, 1.0, 721.0, 1.0, 1.0, 1.0);
    let since_val = gtk::SpinButton::new(Some(&adjustment), 1.0, 0);
    since_val.set_value(10.0);

    let since_row = BoxBuilder::new()
        .orientation(Orientation::Horizontal)
        .spacing(4)
        .margin_top(8)
        .build();

    since_row.add(&gtk::Label::new(Some("Since:")));
    since_row.add(&since_val);

    let unit_selector = gtk::ComboBoxText::new();
    unit_selector.append(Some(UNIT_MINUTES), "Minutes");
    unit_selector.append(Some(UNIT_HOURS), "Hours");
    unit_selector.set_active(Some(0));

    since_row.add(&unit_selector);
    content.add(&since_row);

    let rules_view = HighlighterListView::new();
    rules_view.add_highlighters(get_default_highlighters());
    content.add(rules_view.view());

    dlg.connect_delete_event(move |dlg, _| {
        dlg.hide();
        gtk::Inhibit(true)
    });

    dlg.add_button("_Cancel", ResponseType::Cancel);
    dlg.add_button("_Open", ResponseType::Accept);

    kube_action.connect_activate(move |_,_| {
        match get_pods() {
            Ok(pods) => {
                set_pods(&service_model, pods, include_replicas.get_active());
                dlg.show_all();

                let res = dlg.run();
                dlg.close();
                let since = since_val.get_text().to_string();
                let active_unit = unit_selector.get_active_id().map(|s|s.to_string()).unwrap_or(UNIT_MINUTES.to_string());
                let unit_multiplier = if active_unit.as_str() == UNIT_MINUTES {
                    60
                }else {
                    60 * 60
                };

                let since = since.parse::<u32>().unwrap_or(10) * unit_multiplier;
                let separate_tabs = log_separate_tab_checkbox.get_active();

                if res == ResponseType::Accept {
                    let mut models = vec![];
                    if let Some(iter)  = service_model.get_iter_first() {
                        if let Some((service, active)) = map_model(&service_model, &iter) {
                            if active {
                                models.push(service);
                            }
                        }
                        while service_model.iter_next(&iter) {
                            if let Some((service, active)) = map_model(&service_model, &iter) {
                                if active {
                                    models.push(service);
                                }
                            }
                        }
                    }

                    if separate_tabs {
                        for model in models {
                            tx.send(Msg::CreateTab(CreateLogView::with_rules(LogViewData::Kube(CreateKubeLogData {
                                pods: vec![model],
                                since,
                            }), rules_view.get_highlighter().unwrap()))).expect("Could not send create tab msg");
                        }
                    }else {
                        tx.send(Msg::CreateTab(CreateLogView::with_rules(LogViewData::Kube(CreateKubeLogData {
                            pods: models,
                            since,
                        }), rules_view.get_highlighter().unwrap()))).expect("Could not send create tab msg");
                    }
                }
            }
            Err(e) => {
                error!("Could not get pods: {}", e);
                let dlg = gtk::MessageDialog::new::<ApplicationWindow>(
                    None,
                    DialogFlags::MODAL,
                    MessageType::Error,
                    ButtonsType::Ok,
                    &e.to_string() );
                dlg.run();
                dlg.close();
            }
        }
    });
    Some(kube_action)
}