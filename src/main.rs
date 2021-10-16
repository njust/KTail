#![windows_subsystem = "windows"]

#[macro_use]
extern crate glib;

mod start;
mod model;
mod util;
mod pod_selector;
mod log_view;
mod toolbar;
mod log_text_view;
mod log_file_reader;
mod log_text_contrast;
mod kubernetes_log_reader;
mod highlighters;
mod menu;
mod widget;
mod k8s_client;
mod settings;

use gtk::prelude::*;
use gio::prelude::*;
use gtk::{Application, ApplicationWindow, Button,  Orientation, Label, IconSize, ReliefStyle, AccelGroup, NotebookBuilder};
use gio::{SimpleAction};
use log::{error, info};
use uuid::Uuid;
use std::path::PathBuf;
use glib::Sender;
use crate::pod_selector::{PodSelector};
use std::collections::HashMap;
use crate::log_view::LogView;
use crate::model::{LogViewData, Msg, CreateLogView};
use crate::highlighters::{Highlighter, SEARCH_ID, RULE_TYPE_HIGHLIGHT};
use util::{get_app_icon, send_msg};
use menu::configure_menu;
use crate::menu::create_open_file_dlg_action;

pub fn get_default_highlighters() -> Vec<Highlighter> {
    vec![
        Highlighter {
            id: Uuid::parse_str(SEARCH_ID).unwrap(),
            regex: None,
            color: Some(String::from("rgba(188,150,0,1)")),
            name: Some(String::from("Search")),
            is_system: true,
            rule_type: RULE_TYPE_HIGHLIGHT.to_string(),
            extractor_regex: None,
        },
        Highlighter {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)error|fatal|failed(?-i))\s.*".into()),
            color: Some(String::from("rgba(244,94,94,1)")),
            name: Some(String::from("Error")),
            is_system: false,
            rule_type: RULE_TYPE_HIGHLIGHT.to_string(),
            extractor_regex: Some("((?i)error|fatal|failed(?-i))(?P<text>.*)".to_string()),
        },
        Highlighter {
            id: Uuid::new_v4(),
            regex: Some(r".*\s((?i)warn(?-i))\s.*".into()),
            color: Some(String::from("rgba(207,111,57,1)")),
            name: Some(String::from("Warning")),
            is_system: false,
            rule_type: RULE_TYPE_HIGHLIGHT.to_string(),
            extractor_regex: Some("((?i)warn(?-i))(?P<text>.*)".to_string()),
        }
    ]
}

fn create_tab(data: CreateLogView, tx: Sender<Msg>, id: Uuid, accelerators: &AccelGroup) -> (gtk::Box, LogView) {
    let tx2 = tx.clone();
    let tab_name = data.data.get_name();
    let file_view = LogView::new(data, move |msg| {
        send_msg(&tx2, Msg::LogViewMsg(id, msg));
    }, accelerators);

    let close_btn = Button::from_icon_name(Some("window-close-symbolic"), IconSize::Menu);
    close_btn.set_relief(ReliefStyle::None);

    let tab_header = gtk::Box::new(Orientation::Horizontal, 0);
    tab_header.add(&Label::new(Some(&tab_name)));
    tab_header.add(&close_btn);

    let tx = tx.clone();
    close_btn.connect_clicked(move |_| {
        send_msg(&tx, Msg::CloseTab(id));
    });

    tab_header.show_all();
    (tab_header, file_view)
}

fn main() {
    let mut rt = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build().expect("Could not init scheduler");
    rt.block_on(async move {
        int_main().await;
    })
}

async fn int_main() {
    if let Err(e) = log4rs::init_file("config/log4rs.yaml", Default::default()) {
        error!("Could not init log with log4rs config: {:?}", e);
    }
    info!("Logger initialized");
    info!("Started with args: {:?}", std::env::args());

    let application = Application::new(
        Some("de.ktail"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(move |app| {
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let notebook = NotebookBuilder::new()
            .hexpand(true)
            .vexpand(true)
            .build();

        {
            let tx = tx.clone();
            let te = gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::OTHER_APP, 129);
            notebook.drag_dest_set(gtk::DestDefaults::ALL, &[te], gdk::DragAction::DEFAULT);
            notebook.connect_drag_data_received(move |_, _, _, _, data, _, _| {
                let uris = data.get_uris();
                for uri in uris {
                    let mut parts = uri.split("///");
                    if let Some(file) = parts.nth(1) {
                        // Todo: on linux this check should be more sophisticated
                        // for now it helps with unc paths..
                        let file = if file.starts_with("/") {
                            format!("/{}", file)
                        }else {
                            file.to_string()
                        };

                        // Whitespaces are encoded in file path
                        let file = file.replace("%20", " ");
                        let file = PathBuf::from(file);
                        if let Some(mime) = mime_guess::from_path(&file).first() {
                            if mime.type_() == mime_guess::mime::TEXT  {
                                send_msg(&tx, CreateLogView::new(LogViewData::File(file)));
                            }
                        }
                    }
                }
            });
        }

        let open_action = create_open_file_dlg_action(tx.clone());
        app.add_action(&open_action);

        {
            let tx = tx.clone();
            let next_tab_action = SimpleAction::new("next_tab", None);
            app.add_action(&next_tab_action);
            next_tab_action.connect_activate(move |_,_| {
                send_msg(&tx, Msg::NextTab);
            });
            app.set_accels_for_action("app.next_tab", &["<Primary>Tab"]);
        }


        {
            let tx = tx.clone();
            let prev_tab_action = SimpleAction::new("prev_tab", None);
            app.add_action(&prev_tab_action);
            prev_tab_action.connect_activate(move |_, _| {
                send_msg(&tx, Msg::PrevTab);
            });
            app.set_accels_for_action("app.prev_tab", &["<Primary><Shift>Tab"]);
        }

        {
            let tx = tx.clone();
            let close_current_tab_action = SimpleAction::new("close_current_tab", None);
            app.add_action(&close_current_tab_action);
            close_current_tab_action.connect_activate(move |_, _| {
                send_msg(&tx, Msg::CloseActiveTab);
            });
            app.set_accels_for_action("app.close_current_tab", &["<Primary>W"]);
        }

        if let Some(open_with) = std::env::args().nth(1) {
            if std::path::Path::new(&open_with).exists() {
                let tx = tx.clone();
                let open_with = open_with.clone();
                send_msg(&tx, CreateLogView::new(LogViewData::File(std::path::PathBuf::from(open_with))));
            }
        }

        let pod_selector_tx = tx.clone();
        let pod_selector_tx2 = tx.clone();
        let mut pod_selector = PodSelector::new(move |msg| {
            if let Err(e) = pod_selector_tx.send(Msg::PodSelectorMsg(msg)) {
                error!("Could not send msg: {}", e);
            }
        }, pod_selector_tx2);

        let tx2 = tx.clone();
        app.connect_shutdown(move|_| {
            send_msg(&tx2, Msg::Exit);
        });

        let exit_action = SimpleAction::new("quit", None); {
            let tx = tx.clone();
            exit_action.connect_activate(move |_a, _b| {
                send_msg(&tx, Msg::Exit);
            });
            app.add_action(&exit_action);
        }

        let mut file_views = HashMap::<Uuid, LogView>::new();
        let window = ApplicationWindow::new(app);

        let ag = AccelGroup::new();
        window.add_accel_group(&ag);

        let main = gtk::Box::new(Orientation::Vertical, 0);
        configure_menu(tx.clone(), &app, &window, &main);
        window.add(&main);
        main.add(&notebook);

        let tx = tx.clone();
        rx.attach(None, move |msg| {
            match msg {
                Msg::LogViewMsg(id, msg) => {
                    if let Some(tab) = file_views.get_mut(&id) {
                        tab.update(msg);
                    }
                }
                Msg::CloseTab(id) => {
                    if let Some(tab) = file_views.get_mut(&id) {
                        tab.close();
                        notebook.detach_tab(tab.view());
                        file_views.remove(&id);
                    }
                }
                Msg::Exit => {
                    for tab in file_views.values() {
                        notebook.detach_tab(tab.view());
                    }
                    file_views.clear();
                    gio::Application::get_default()
                        .expect("no default Application!")
                        .quit();
                }
                Msg::CreateTab(tab) => {
                    let id = Uuid::new_v4();
                    let (tab_header, file_view) = create_tab(tab, tx.clone(), id, &ag);
                    let new_tab_index = notebook.append_page(file_view.view(), Some(&tab_header));
                    // notebook.set_tab_detachable(file_view.view(), true);
                    file_views.insert(id, file_view);
                    notebook.show_all();
                    notebook.set_current_page(Some(new_tab_index));
                }
                Msg::NextTab => {
                    notebook.next_page();
                }
                Msg::PrevTab => {
                    notebook.prev_page();
                }
                Msg::CloseActiveTab => {
                    if let Some(current) = notebook.get_nth_page(notebook.get_current_page()) {
                        let active_tab = file_views.iter().find(|(_, file_view)| {
                            let view = file_view.view().upcast_ref::<gtk::Widget>();
                            view == &current
                        }).map(|(id, _)| id.clone());

                        if let Some(id) = active_tab {
                            if let Some(tab) = file_views.get_mut(&id) {
                                tab.close();
                                notebook.detach_tab(tab.view());
                                file_views.remove(&id);
                            }
                        }
                    }
                }
                Msg::PodSelectorMsg(msg) => {
                    pod_selector.update(msg)
                }
            }
            glib::Continue(true)
        });

        window.set_title("Log Viewer");
        window.set_default_size(1280, 600);

        let icon = get_app_icon();
        window.set_icon(
            Some(&icon)
        );

        window.show_all();
    });

    application.run(&[]);
}


