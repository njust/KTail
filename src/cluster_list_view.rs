use std::path::PathBuf;
use std::rc::Rc;
use gtk4_helper::prelude::{Command, MsgHandler};

use gtk4_helper::{
    gtk,
    gio,
    gtk::prelude::*,
    component::Component,
    model::prelude::*,
};
use gtk4_helper::gio::ListStore;
use gtk4_helper::gtk::{ColumnView, Orientation, ScrolledWindow};
use itertools::Itertools;
use crate::{ApplicationWindow, column_view_helper};
use crate::column_view_helper::ButtonOptions;
use crate::config::{CONFIG};
use crate::k8s_client::KubeConfig;
use crate::result::AppResult;
use crate::util::{WidgetLoadingWrapper, show_and_log_error};



pub struct ClusterListView {
    container: gtk::Box,
    config_list_data: gio::ListStore,
    context_list_data: gio::ListStore,
    namespace_list_data: gio::ListStore,
    namespace_list_view: WidgetLoadingWrapper<ScrolledWindow>,
    app_wnd: Rc<ApplicationWindow>,
}

#[derive(Clone)]
pub enum ClusterListViewMsg {
    Init,
    ConfigsLoaded(Vec<KubeConfigViewData>),
    ClusterSelected(NamespaceViewData),
    ContextSelected(ContextViewData),
    ConfigSelected(KubeConfigViewData),
    AddConfig,
    RemoveConfig(u32),
    ConfigAdded(Option<PathBuf>),
    NamespacesLoaded(AppResult<Vec<NamespaceViewData>>),
}

#[model]
#[derive(Clone)]
pub struct KubeConfigViewData {
    #[field]
    pub path: String,
}

#[model]
#[derive(Clone)]
pub struct ContextViewData {
    #[field]
    pub name: String,
    #[field]
    pub config_path: String,
}

#[model]
#[derive(Clone)]
pub struct NamespaceViewData {
    #[field]
    pub name: String,
    #[field]
    pub context: String,
    #[field]
    pub config_path: String,
}

pub struct ClusterListInputData {
    pub app_wnd: Rc<ApplicationWindow>,
}

impl Component for ClusterListView {
    type Msg = ClusterListViewMsg;
    type View = gtk::Box;
    type Input = ClusterListInputData;

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, input: Option<Self::Input>) -> Self {
        let input = input.expect("Cluster list view input data is required!");
        let container = gtk::Box::new(Orientation::Vertical, 0);
        container.set_hexpand(true);

        let toolbar = gtk::Box::new(Orientation::Horizontal, 4);
        container.append(&toolbar);

        let add_btn = gtk::Button::builder()
            .label("Add config")
            .margin_start(4)
            .margin_end(4)
            .margin_top(4)
            .margin_bottom(4)
            .build();


        let tx = sender.clone();
        add_btn.connect_clicked(move |_| {
            tx(ClusterListViewMsg::AddConfig);
        });

        toolbar.append(&add_btn);

        let (config_list_view, config_list_data) = kube_config_list(sender.clone());
        let config_scroll_wnd = gtk::ScrolledWindowBuilder::new()
            .child(&config_list_view)
            .build();

        let (context_list_view, context_list_data) = cluster_context_list(sender.clone());
        let ctx_scroll_wnd = gtk::ScrolledWindowBuilder::new()
            .vexpand(true)
            .child(&context_list_view)
            .build();

        let pane1 = gtk::PanedBuilder::new()
            .orientation(Orientation::Vertical)
            .position(110)
            .vexpand(true)
            .start_child(&config_scroll_wnd)
            .end_child(&ctx_scroll_wnd)
            .build();

        let (namespace_list_view, namespace_list_data) = namespace_list(sender.clone());
        let namespace_wnd = WidgetLoadingWrapper::new(gtk::ScrolledWindowBuilder::new()
            .vexpand(true)
            .child(&namespace_list_view)
            .build());

        let pane2 = gtk::PanedBuilder::new()
            .orientation(Orientation::Vertical)
            .position(230)
            .vexpand(true)
            .start_child(&pane1)
            .end_child(namespace_wnd.container())
            .build();

        container.append(&pane2);

        let tx = sender.clone();
        container.connect_realize(move |_| {
            tx(ClusterListViewMsg::Init)
        });

        Self {
            container,
            config_list_data,
            context_list_data,
            namespace_list_data,
            namespace_list_view: namespace_wnd,
            app_wnd: input.app_wnd
        }
    }

    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            ClusterListViewMsg::Init => {
                return self.run_async(load_configs());
            }
            ClusterListViewMsg::ConfigsLoaded(configs) => {
                for config in configs {
                    let obj = KubeConfigViewData::to_object(&config);
                    self.config_list_data.append(&obj);
                }
            }
            ClusterListViewMsg::ClusterSelected(_) => {}
            ClusterListViewMsg::AddConfig => {
                return self.run_async_local(add_config_file(self.app_wnd.clone()));
            }
            ClusterListViewMsg::ConfigAdded(path) => {
                if let Some(path) = path {
                    if let Some(path) = path.to_str() {
                        let config = KubeConfigViewData {
                            path: path.to_string()
                        };
                        let config = KubeConfigViewData::to_object(&config);
                        self.config_list_data.append(&config);
                    }
                }
            }
            ClusterListViewMsg::ContextSelected(ctx) => {
                self.namespace_list_view.set_is_loading(true);
                return self.run_async(load_namespaces(ctx.config_path, ctx.name));
            }
            ClusterListViewMsg::ConfigSelected(cfg) => {
                let path = cfg.path;
                if let Ok(cfg) = KubeConfig::load(&path) {
                    self.context_list_data.remove_all();
                    for context in cfg.contexts {
                        let ctx = ContextViewData {
                            name: context.name.clone(),
                            config_path: path.clone(),
                        };
                        let ctx = ContextViewData::to_object(&ctx);
                        self.context_list_data.append(&ctx);
                    }
                }
            }
            ClusterListViewMsg::NamespacesLoaded(data) => {
                self.namespace_list_view.set_is_loading(false);
                match data {
                    Ok(data) => {
                        self.namespace_list_data.remove_all();
                        for namespace in data {
                            let obj = NamespaceViewData::to_object(&namespace);
                            self.namespace_list_data.append(&obj);
                        }
                    }
                    Err(e) => {
                        show_and_log_error("Failed to load namespaces", &e.to_string(), Some(&*self.app_wnd.clone()));
                    }
                }
            }
            ClusterListViewMsg::RemoveConfig(pos) => {
                if let Some(item) = self.config_list_data.item(pos) {
                    let cfg_item: KubeConfigViewData = KubeConfigViewData::from_object(&item);
                    if let Ok(mut cfg) = CONFIG.lock() {
                        if let Some((pos,_)) = cfg.k8s_configs.iter().find_position(|a| *a == &cfg_item.path) {
                            cfg.k8s_configs.remove(pos);
                        }
                    }
                }
                self.config_list_data.remove(pos);

            }
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        &self.container
    }
}

async fn load_namespaces(config_path: String, context: String) -> ClusterListViewMsg {
    let client = crate::log_stream::k8s_client_with_timeout(&config_path, &context);
    let res = client.namespaces().await.and_then(|data| {
        Ok(data.iter().map(|data| NamespaceViewData {
            name: data.metadata.name.as_ref().unwrap().clone(),
            config_path: config_path.clone(),
            context: context.clone()
        }).collect())
    }).map_err(|e| e.into());
    ClusterListViewMsg::NamespacesLoaded(res)
}

fn kube_config_list<T: MsgHandler<ClusterListViewMsg> + Clone>(sender: T) -> (ColumnView, ListStore) {
    let (column_view, list_store) =
        column_view_helper::create_column_view(KubeConfigViewData::static_type(), column_view_helper::single_selection_model);
    column_view.set_single_click_activate(true);
    column_view_helper::create_label_column(&column_view, KubeConfigViewData::static_type(), KubeConfigViewData::path, "Config", column_view_helper::string_sorter);
    let tx = sender.clone();
    column_view_helper::create_button_column(&column_view, "", move |pos| {
        tx(ClusterListViewMsg::RemoveConfig(pos));
    }, ButtonOptions {
        label: None,
        image: Some("edit-delete-symbolic"),
    });

    let tx = sender;
    column_view.connect_activate(move |view, pos| {
        if let Some(item) = view.model().as_ref()
            .and_then(|model| model.item(pos))
        {
            let config: KubeConfigViewData = KubeConfigViewData::from_object(&item);
            tx(ClusterListViewMsg::ConfigSelected(config));
        }
    });

    (column_view, list_store)
}

fn cluster_context_list<T: MsgHandler<ClusterListViewMsg>>(tx: T) -> (ColumnView, ListStore) {
    let (column_view, list_store) =
        column_view_helper::create_column_view(ContextViewData::static_type(), column_view_helper::single_selection_model);
    column_view.set_single_click_activate(true);
    column_view_helper::create_label_column(&column_view, ContextViewData::static_type(), ContextViewData::name, "Context", column_view_helper::string_sorter);

    column_view.connect_activate(move |view, pos| {
        if let Some(item) = view.model().as_ref()
            .and_then(|model| model.item(pos))
        {
            let ctx: ContextViewData = ContextViewData::from_object(&item);
            tx(ClusterListViewMsg::ContextSelected(ctx));
        }
    });

    (column_view, list_store)
}

fn namespace_list<T: MsgHandler<ClusterListViewMsg>>(tx: T) -> (ColumnView, ListStore) {
    let (column_view, list_store) =
        column_view_helper::create_column_view(NamespaceViewData::static_type(), column_view_helper::single_selection_model);
    column_view.set_single_click_activate(true);
    column_view_helper::create_label_column(&column_view, NamespaceViewData::static_type(), NamespaceViewData::name, "Namespace", column_view_helper::string_sorter);

    column_view.connect_activate(move |view, pos| {
        if let Some(item) = view.model().as_ref()
            .and_then(|model| model.item(pos))
        {
            let ctx: NamespaceViewData = NamespaceViewData::from_object(&item);
            tx(ClusterListViewMsg::ClusterSelected(ctx));
        }
    });

    (column_view, list_store)
}

async fn add_config_file(app_wnd: Rc<ApplicationWindow>) -> ClusterListViewMsg {
    let dlg = gtk::FileChooserDialog::builder()
        .title("Select config")
        .modal(true)
        .transient_for(&*app_wnd)
        .action(gtk::FileChooserAction::Open)
        .build();

    dlg.add_buttons(&[("Select", gtk::ResponseType::Ok), ("Cancel", gtk::ResponseType::Cancel)]);
    let path = if dlg.run_future().await == gtk::ResponseType::Ok {
        dlg.file().and_then(|sel| sel.path())
    } else {
        None
    };

    if let Some(path) = path.as_ref().and_then(|p| p.to_str()) {
        if let Ok(mut cfg) = CONFIG.lock() {
            cfg.k8s_configs.push(path.to_string());
        }
    }

    dlg.close();
    ClusterListViewMsg::ConfigAdded(path)
}

async fn load_configs() -> ClusterListViewMsg {
    let cfgs = if let Ok(cfg) = CONFIG.lock() {
        cfg.k8s_configs.iter().map(|file| {
            KubeConfigViewData {
                path: file.clone()
            }
        }).collect()
    } else {
        vec![]
    };

    ClusterListViewMsg::ConfigsLoaded(cfgs)
}