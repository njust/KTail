use std::rc::Rc;
use gtk4_helper::{
    prelude::*,
    gtk,
    glib
};

#[model]
#[derive(Clone)]
pub struct PodViewData {
    #[field]
    pub name: String,
    #[field]
    container_names: String,
}

impl PodViewData {
    pub fn containers(&self) -> Vec<String> {
        self.container_names.split(";").into_iter().map(|s|s.to_string()).collect()
    }
}

use gtk4_helper::{
    gio,
    model::prelude::*,
};

use crate::column_view_helper;
use crate::cluster_list_view::NamespaceViewData;
use crate::result::AppResult;
use crate::util::{show_and_log_error, WidgetLoadingWrapper};

#[derive(Clone, Debug)]
pub enum PodListViewMsg {
    Loaded(AppResult<Vec<PodViewData>>),
    PodSelected(Vec<PodViewData>),
    ClusterSelected(NamespaceViewData)
}

pub struct PodListView {
    pod_list_data: gio::ListStore,
    pod_list_view: WidgetLoadingWrapper<gtk::ScrolledWindow>,
    app_wnd: Rc<ApplicationWindow>,
}

impl Component for PodListView {
    type Msg = PodListViewMsg;
    type View = gtk::Box;
    type Input = Rc<ApplicationWindow>;

    fn create<T: MsgHandler<Self::Msg> + Clone>(sender: T, input: Option<Self::Input>) -> Self {
        let (column_view, list_store) =
            column_view_helper::create_column_view(PodViewData::static_type(), column_view_helper::multi_selection_model);

        let tx = sender.clone();
        column_view.connect_activate(move |view, _| {
            if let Some(model) = view.model() {
                let sel = model.selection();
                let mut selected_pods = vec![];
                for idx in 0..sel.size() {
                    let sel = sel.nth(idx as u32);
                    if let Some(sel_item) = model.item(sel) {
                        let sel_pod: PodViewData = PodViewData::from_object(&sel_item);
                        selected_pods.push(sel_pod);
                    }
                }
                tx(PodListViewMsg::PodSelected(selected_pods))
            }
        });

        column_view_helper::create_label_column(&column_view, PodViewData::static_type(), PodViewData::name, "Pod", column_view_helper::string_sorter);

        let pod_list_view = WidgetLoadingWrapper::new(gtk::ScrolledWindowBuilder::new()
            .vexpand(true)
            .child(&column_view)
            .build());

        let app_wnd = input.expect("Input is required!");
        Self {
            pod_list_view,
            pod_list_data: list_store,
            app_wnd
        }
    }

    fn update(&mut self, msg: Self::Msg) -> Command<Self::Msg> {
        match msg {
            PodListViewMsg::Loaded(res) => {
                self.pod_list_view.set_is_loading(false);
                match res {
                    Ok(pvd) => {
                        for pod_data in pvd {
                            let obj = pod_data.to_object();
                            self.pod_list_data.append(&obj);
                        }
                    }
                    Err(e) => {
                        show_and_log_error("Failed to load pods", &e.to_string(), Some(&*self.app_wnd.clone()));
                    }
                }
            }
            PodListViewMsg::ClusterSelected(cluster) => {
                self.pod_list_view.set_is_loading(true);
                self.pod_list_data.remove_all();
                return self.run_async(load_data(cluster));
            }
            PodListViewMsg::PodSelected(_) => {}
        }
        Command::None
    }

    fn view(&self) -> &Self::View {
        self.pod_list_view.container()
    }
}

async fn load_data(cluster: NamespaceViewData) -> PodListViewMsg {
    let client = crate::log_stream::k8s_client_with_timeout(&cluster.config_path, &cluster.context);
    let res = client.pods(&cluster.name).await.and_then(|pods| {
        Ok(pods.into_iter().map(|p| {
            //TODO: Currently gtk helper model does not support Vec<String>
            let container_names = p.spec.containers.iter().map(|c| c.name.as_str()).collect::<Vec<&str>>().join(";");
            let pod_name = p.metadata.name.unwrap_or("failed".to_string());
            PodViewData {
                container_names,
                name: pod_name,
            }
        }).collect())
    }).map_err(|e| e.into());
    PodListViewMsg::Loaded(res)
}