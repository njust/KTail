use gtk::prelude::*;
use gtk::{ApplicationWindow, HeaderBar, ResponseType, Orientation, TreeStore, WindowPosition, TreeIter, SortColumn, SortType, ScrolledWindow, DialogFlags, MessageType, ButtonsType};
use std::rc::Rc;
use gio::{SimpleAction};
use log::{error};

use crate::util::{create_col, ColumnType};
use glib::Sender;
use k8s_client::{KubeConfig, KubeClient};
use crate::model::{Msg, LogTextViewData, CreateKubeLogData};

fn map_model(model: &TreeStore, iter: &TreeIter) -> Option<(String, bool)> {
    let name = model.get_value(iter, 0).get::<String>().unwrap_or(None)?;
    let active = model.get_value(iter, 1).get::<bool>().unwrap_or(None)?;
    Some((name, active))
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
    include_replicas.set_active(true);
    content.add(&include_replicas);

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
        let pods = glib::MainContext::default().block_on(async move  {
            match KubeClient::load_conf(None) {
                Ok(c) => {
                    c.pods().await
                }
                Err(e) => {
                    Err(e)
                }
            }
        });

        match pods {
            Ok(pods) => {
                service_model.clear();
                for pod in pods {
                    if let Some(name) = pod.metadata.name {
                        service_model.insert_with_values(None, None, &[0, 1], &[&name, &false]);
                    }
                }

                dlg.show_all();

                let res = dlg.run();
                dlg.close();
                let since = since_val.get_text().to_string();
                let since = since.parse::<u32>().unwrap_or(4);
                let separate_tabs = log_separate_tab_checkbox.get_active();
                let include_replicas = include_replicas.get_active();

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

                    let pods = if include_replicas {
                        models.iter().map(|name| {
                            let parts = name.split("-").collect::<Vec<&str>>();
                            let len = parts.len();
                            parts.into_iter().take(len -2).collect::<Vec<&str>>().join("-")
                        }).collect::<Vec<String>>()
                    }else {
                        models
                    };

                    if separate_tabs {
                        for model in pods {
                            tx.send(Msg::CreateTab(LogTextViewData::Kube(CreateKubeLogData {
                                pods: vec![model],
                                since
                            }))).expect("Could not send create tab msg");
                        }
                    }else {
                        tx.send(Msg::CreateTab(LogTextViewData::Kube(CreateKubeLogData {
                            pods,
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