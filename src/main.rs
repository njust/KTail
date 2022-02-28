#![windows_subsystem = "windows"]

use std::rc::Rc;
use gtk4_helper::{
    prelude::*,
    gtk,
    glib,
    gio
};
use gtk4_helper::gtk::Orientation;
use crate::cluster_list_view::{ClusterListInputData, ClusterListView, ClusterListViewMsg};
use crate::config::{CONFIG};
use crate::gtk::Inhibit;
use crate::log_view::{LogView, LogViewMsg};
use flexi_logger::{Duplicate, FileSpec, Logger, WriteMode, detailed_format};

mod k8s_client;
mod log_stream;
mod column_view_helper;
mod pod_list_view;
mod log_view;
mod cluster_list_view;
mod util;
mod config;
mod log_text_contrast;
mod log_overview;
mod result;
mod dirs;

use crate::pod_list_view::{PodListView, PodListViewMsg};

pub enum AppMsg {
    PodListViewMsg(PodListViewMsg),
    LogViewMsg(LogViewMsg),
    ClusterListViewMsg(ClusterListViewMsg),
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
    window.connect_close_request(|_| {
        log::info!("Stopping");
        if let Err(e) = CONFIG.lock().map(|cfg| cfg.save()) {
            log::error!("Could not save config: {}", e);
        }
        Inhibit(false)
    });
    window.set_title(Some("KTail"));
    window.set_default_size(1600, 768);
    let global_actions = Rc::new(gio::SimpleActionGroup::new());
    window.insert_action_group("app", Some(&*global_actions));
    let window = Rc::new(window);

    let (sender, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);
    let tx = sender.clone();
    let mut pod_list = PodListView::new_with_data(move |m| {
        tx.send(AppMsg::PodListViewMsg(m)).expect("Could not send msg");
    }, window.clone());

    let tx = sender.clone();
    let mut log_view = LogView::new_with_data(move |m| {
        tx.send(AppMsg::LogViewMsg(m)).expect("Could not send log view msg");
    }, global_actions.clone());

    let horizontal_split = gtk::Paned::new(Orientation::Horizontal);
    horizontal_split.set_position(250);
    horizontal_split.set_vexpand(true);

    let tx = sender.clone();
    let mut cluster_list = ClusterListView::new_with_data(move |m| {
        tx.send(AppMsg::ClusterListViewMsg(m)).expect("Could not send cluster list view msg");
    }, ClusterListInputData {app_wnd: window.clone()});

    let vertical_split = gtk::Paned::new(Orientation::Vertical);
    vertical_split.set_position(400);
    vertical_split.set_start_child(cluster_list.view());
    vertical_split.set_end_child(pod_list.view());

    horizontal_split.set_start_child(&vertical_split);
    horizontal_split.set_end_child(log_view.view());

    rx.attach(None, move |msg| {
        match msg {
            AppMsg::ClusterListViewMsg(msg) => {
                if let ClusterListViewMsg::ClusterSelected(sel) = &msg  {
                    pod_list.update(PodListViewMsg::ClusterSelected(sel.clone()));
                    log_view.update(LogViewMsg::ContextSelected(sel.clone()));
                }
                cluster_list.update(msg);
            }
            AppMsg::PodListViewMsg(msg) => {
                if let PodListViewMsg::PodSelected(sel) = &msg {
                    log_view.update(LogViewMsg::PodSelected(sel.clone()));
                }
                pod_list.update(msg);
            }
            AppMsg::LogViewMsg(msg) => {
                log_view.update(msg);
            }
        }
        glib::Continue(true)
    });

    application.set_accels_for_action("app.search", &["<Ctrl>F"]);
    application.set_accels_for_action("app.scroll", &["<Ctrl>Q"]);
    application.set_accels_for_action("app.prevMatch", &["<Ctrl>P"]);
    application.set_accels_for_action("app.nextMatch", &["<Ctrl>N"]);
    application.set_accels_for_action("app.toggleWrapText", &["<Ctrl>W"]);
    application.set_accels_for_action("app.showPodNames", &["<Alt>P"]);
    application.set_accels_for_action("app.showContainerNames", &["<Alt>C"]);
    application.set_accels_for_action("app.showTimestamps", &["<Alt>T"]);
    window.set_child(Some(&horizontal_split));
    window.show();
}

fn main() {

    if let Err(e) = Logger::try_with_str("info")
        .and_then(|l| l
            .format_for_files(detailed_format)
            .log_to_file(FileSpec::default()
                .directory(dirs::log_dir())
            )
            .duplicate_to_stderr(Duplicate::Error)
            .create_symlink(dirs::log_dir().join("ktail.log"))
            .print_message()
            .write_mode(WriteMode::Direct)
            .start()) {
        log::error!("Could not initialize logger: {}", e);
    }

    log::info!("Starting");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let application =
            gtk::Application::new(Some("de.ktail"), Default::default());

        application.connect_activate(|app| {
            build_ui(app);
        });

        application.run();
    });
}