#![windows_subsystem = "windows"]

#[macro_use]
extern crate glib;

mod model;
mod util;
mod pod_selection;
mod log_view;
mod toolbar;
mod log_text_view;
mod log_file_reader;
mod kubernetes_log_reader;
mod highlighters;

use gtk::prelude::*;
use gio::prelude::*;
use gtk::{Application, ApplicationWindow, Button, HeaderBar, Notebook, MenuButton, FileChooserDialog, FileChooserAction, ResponseType, Orientation, Label, IconSize, ReliefStyle, AccelGroup};
use gio::{SimpleAction};
use log::{error, info};
use uuid::Uuid;
use std::path::PathBuf;
use glib::Sender;
use crate::pod_selection::create_open_kube_action;
use std::collections::HashMap;
use crate::log_view::LogView;
use crate::model::{LogTextViewData, Msg};


fn create_tab(data: LogTextViewData, tx: Sender<Msg>, id: Uuid, accelerators: &AccelGroup) -> (gtk::Box, LogView) {
    let tx2 = tx.clone();
    let tab_name = data.get_name();
    let file_view = LogView::new(data, move |msg| {
        tx2.send(Msg::WorkbenchMsg(id, msg)).expect("Could not send msg");
    }, accelerators);

    let close_btn = Button::from_icon_name(Some("window-close-symbolic"), IconSize::Menu);
    close_btn.set_relief(ReliefStyle::None);

    let tab_header = gtk::Box::new(Orientation::Horizontal, 0);
    tab_header.add(&Label::new(Some(&tab_name)));
    tab_header.add(&close_btn);

    let tx = tx.clone();
    close_btn.connect_clicked(move |_| {
        tx.send(Msg::CloseTab(id)).expect("Could not send close tab msg");
    });

    tab_header.show_all();
    (tab_header, file_view)
}

fn create_open_file_dlg_action(tx: Sender<Msg>) -> SimpleAction {
    let open_action = SimpleAction::new("open", None);
    open_action.connect_activate(move |_a, _b| {
        let dialog = FileChooserDialog::new::<ApplicationWindow>(Some("Open File"), None, FileChooserAction::Open);
        dialog.add_button("_Cancel", ResponseType::Cancel);
        dialog.add_button("_Open", ResponseType::Accept);
        let res = dialog.run();
        dialog.close();
        if res == ResponseType::Accept {
            if let Some(file_path) = dialog.get_filename() {
                tx.send(Msg::CreateTab(LogTextViewData::File(file_path))).expect("Could not send create tab msg");
            }
        }
    });
    open_action
}

fn main() {
    let mut rt = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build().unwrap();
    rt.block_on(async move {
        int_main().await;
    })
}

async fn int_main() {
    if let Err(e) = log4rs::init_file("config/log4rs.yaml", Default::default()) {
        error!("Could not init log with log4rs config: {:?}", e);
    }
    info!("Logger initialized");

    let application = Application::new(
        Some("de.njust.ktail"),
        Default::default(),
    ).expect("failed to initialize GTK application");

    application.connect_activate(move |app| {
        let menu_model = gio::Menu::new();
        let (tx, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
        let notebook = Notebook::new();
        {
            let tx = tx.clone();
            let te = gtk::TargetEntry::new("text/uri-list", gtk::TargetFlags::OTHER_APP, 129);
            notebook.drag_dest_set(gtk::DestDefaults::ALL, &[te], gdk::DragAction::DEFAULT);
            notebook.connect_drag_data_received(move |_, _, _, _, data, _, _| {
                let uris = data.get_uris();
                for uri in uris {
                    let mut parts = uri.split("///");
                    if let Some(file) = parts.nth(1) {
                        // Whitespaces are encoded in file path
                        let file = file.replace("%20", " ");
                        let file = PathBuf::from(file);
                        if let Some(mime) = mime_guess::from_path(&file).first() {
                            if mime.type_() == mime_guess::mime::TEXT  {
                                tx.send(Msg::CreateTab(LogTextViewData::File(file))).expect("Could not send");
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
                tx.send(Msg::NextTab);
            });
            app.set_accels_for_action("app.next_tab", &["<Primary>Tab"]);
        }


        {
            let tx = tx.clone();
            let prev_tab_action = SimpleAction::new("prev_tab", None);
            app.add_action(&prev_tab_action);
            prev_tab_action.connect_activate(move |_, _| {
                tx.send(Msg::PrevTab);
            });
            app.set_accels_for_action("app.prev_tab", &["<Primary><Shift>Tab"]);
        }


        if let Some(kube_action) = create_open_kube_action(tx.clone()) {
            app.add_action(&kube_action);
            app.set_accels_for_action("app.kube", &["<Primary>K"]);
            menu_model.append_item(&gio::MenuItem::new(Some("Kube"), Some("app.kube")));
        }

        let tx2 = tx.clone();
        app.connect_shutdown(move|_| {
            tx2.send(Msg::Exit).expect("Could not send exit msg");
        });

        let exit_action = SimpleAction::new("quit", None); {
            let tx = tx.clone();
            exit_action.connect_activate(move |_a, _b| {
                tx.send(Msg::Exit).expect("Could not send exit msg");
            });
            app.add_action(&exit_action);
        }

        let mut file_views = HashMap::<Uuid, LogView>::new();
        let window = ApplicationWindow::new(app);
        let ag = AccelGroup::new();
        window.add_accel_group(&ag);

        notebook.set_hexpand(true);
        notebook.set_vexpand(true);
        window.add(&notebook);

        let tx = tx.clone();
        rx.attach(None, move |msg| {
            match msg {
                Msg::WorkbenchMsg(id, msg) => {
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
                    notebook.append_page(file_view.view(), Some(&tab_header));
                    // notebook.set_tab_detachable(file_view.view(), true);
                    file_views.insert(id, file_view);
                    notebook.show_all();
                }
                Msg::NextTab => {
                    notebook.next_page();
                }
                Msg::PrevTab => {
                    notebook.prev_page();
                }
            }
            glib::Continue(true)
        });

        window.set_title("Log Viewer");
        window.set_default_size(1280, 600);

        app.set_accels_for_action("app.open", &["<Primary>O"]);
        menu_model.append_item(&gio::MenuItem::new(Some("Open"), Some("app.open")));
        menu_model.append_item(&gio::MenuItem::new(Some("Quit"), Some("app.quit")));

        let menu_button = MenuButton::new();
        menu_button.set_relief(ReliefStyle::None);
        menu_button.set_popup(Some(&gtk::Menu::from_model(&menu_model)));
        menu_button.set_image(Some(&gtk::Image::from_icon_name(Some("open-menu-symbolic"), IconSize::Menu)));

        let header_bar = HeaderBar::new();
        header_bar.pack_end(&menu_button);
        header_bar.set_show_close_button(true);
        header_bar.set_title(Some("Log viewer"));

        window.set_titlebar(Some(&header_bar));
        window.show_all();
    });

    application.run(&[]);
}
