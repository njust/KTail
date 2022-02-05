#![windows_subsystem = "windows"]

use std::rc::Rc;
use gtk4_helper::{
    prelude::*,
    gtk,
    glib
};
use gtk4_helper::gtk::Orientation;
use crate::cluster_list_view::{ClusterListInputData, ClusterListView, ClusterListViewMsg};
use crate::config::{CONFIG};
use crate::gtk::Inhibit;
use crate::log_view::{LogView, LogViewMsg};

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

use crate::pod_list_view::{PodListView, PodListViewMsg};

pub enum AppMsg {
    PodListViewMsg(PodListViewMsg),
    LogViewMsg(LogViewMsg),
    ClusterListViewMsg(ClusterListViewMsg),
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);
    window.connect_close_request(|_| {
        println!("Saving config");
        if let Err(e) = CONFIG.lock().map(|cfg| cfg.save()) {
            eprintln!("Could not save config: {}", e);
        }
        Inhibit(false)
    });
    window.set_title(Some("KTail"));
    window.set_default_size(1600, 768);
    let window = Rc::new(window);

    let (sender, rx) = glib::MainContext::channel(glib::PRIORITY_DEFAULT);

    let tx = sender.clone();
    let mut pod_list = PodListView::new(move |m| {
        tx.send(AppMsg::PodListViewMsg(m)).expect("Could not send msg");
    });

    let tx = sender.clone();
    let mut log_view = LogView::new(move |m| {
        tx.send(AppMsg::LogViewMsg(m)).expect("Could not send log view msg");
    });

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

    window.set_child(Some(&horizontal_split));
    window.show();
}

fn main() {
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