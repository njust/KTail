use gtk::prelude::*;
use gtk::{ApplicationWindow, HeaderBar, ResponseType, Orientation, TreeStore, WindowPosition, TreeIter, SortColumn, SortType, ScrolledWindow, DialogFlags, MessageType, ButtonsType};
use std::rc::Rc;
use gio::{SimpleAction};
use log::{error};

use crate::util::{create_col, ColumnType};
use glib::Sender;
use k8s_client::{KubeConfig, KubeClient, Pod};
use crate::model::{Msg, LogTextViewData, CreateKubeLogData};
use std::error::Error;
use std::collections::{HashSet};

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
    let dlg = gtk::Dialog::new();
    let service_model = Rc::new(TreeStore::new(&[glib::Type::String, glib::Type::Bool]));
    service_model.set_sort_column_id(SortColumn::Index(0), SortType::Ascending);
    let list = gtk::TreeView::with_model(&*service_model);

    list.append_column(&create_col(Some("Add"), 1, ColumnType::Bool, service_model.clone()));
    list.append_column(&create_col(Some("Pod"), 0, ColumnType::String, service_model.clone()));

    dlg.set_position(WindowPosition::CenterOnParent);
    dlg.set_default_size(450, 400);
    let header_bar = HeaderBar::new();
    header_bar.set_show_close_button(true);
    header_bar.set_title(Some("Pods"));
    dlg.set_titlebar(Some(&header_bar));
    dlg.set_modal(true);

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

    let since_row = gtk::Box::new(Orientation::Horizontal, 0);
    since_row.set_spacing(4);
    let adjustment = gtk::Adjustment::new(4.0, 1.0, 721.0, 1.0, 1.0, 1.0);
    let since_val = gtk::SpinButton::new(Some(&adjustment), 1.0, 0);
    since_val.set_value(4.0);
    since_row.add(&gtk::Label::new(Some("Since hours:")));
    since_row.add(&since_val);
    since_row.set_margin_top(8);

    content.add(&since_row);

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
                let since = since.parse::<u32>().unwrap_or(4);
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
                            tx.send(Msg::CreateTab(LogTextViewData::Kube(CreateKubeLogData {
                                pods: vec![model],
                                since
                            }))).expect("Could not send create tab msg");
                        }
                    }else {
                        tx.send(Msg::CreateTab(LogTextViewData::Kube(CreateKubeLogData {
                            pods: models,
                            since
                        }))).expect("Could not send create tab msg");
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