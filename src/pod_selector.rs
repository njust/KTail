use gtk::prelude::*;
use gtk::{Orientation, TreeStore, WindowPosition, TreeIter, SortColumn, SortType, HeaderBarBuilder, BoxBuilder, Align, ScrolledWindowBuilder, Dialog, ComboBoxTextBuilder, ComboBoxText};
use std::rc::Rc;
use log::{error, info};

use crate::util::{create_col, ColumnType, add_css_with_name, show_error_msg};
use glib::Sender;
use crate::k8s_client::{KubeConfig, KubeClient, Pod, Namespace, ClientOptions};
use crate::model::{Msg, LogViewData, CreateKubeLogData, CreateLogView, PodSelectorMsg};
use std::error::Error;
use std::collections::{HashSet};
use std::time::Duration;
use crate::highlighters::HighlighterListView;
use crate::get_default_highlighters;

const UNIT_MINUTES: &'static str = "UNIT_MINUTES";
const UNIT_HOURS: &'static str = "UNIT_HOURS";

fn map_model(model: &TreeStore, iter: &TreeIter) -> Option<(String, bool)> {
    let name = model.get_value(iter, 0).get::<String>().unwrap_or(None)?;
    let active = model.get_value(iter, 1).get::<bool>().unwrap_or(None)?;
    Some((name, active))
}

pub struct PodSelector {
    dlg: Dialog,
    kube_client: Option<KubeClient>,
    separate_tabs: bool,
    since_multiplier: u32,
    since: u32,
    selected_namespace: Option<String>,
    selected_cluster: Option<String>,
    pods_model: Rc<TreeStore>,
    namespace_model: gtk::ListStore,
    namespace_selector: ComboBoxText,
    cluster_selector: ComboBoxText,
    rules_view: HighlighterListView,
    include_replicas: bool,
    msg: Sender<Msg>, //TODO: get rid of this
}

impl PodSelector {
    pub fn new<T>(tx: T, msg: Sender<Msg>) -> Self
    where T: Fn(PodSelectorMsg) + 'static + Clone
    {
        let header_bar = HeaderBarBuilder::new()
            .show_close_button(true)
            .title("Pods")
            .build();

        let dlg = gtk::DialogBuilder::new()
            .window_position(WindowPosition::CenterOnParent)
            .default_width(540)
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
        let col = gtk::BoxBuilder::new()
            .spacing(5)
            .orientation(Orientation::Vertical)
            .build();
        content.add(&col);

        let cluster_selector = ComboBoxTextBuilder::new()
            .build();
        {
            let tx = tx.clone();
            cluster_selector.connect_changed(move|cb|{
                if let Some(active) = cb.get_active_id().map(|gs|gs.to_string()) {
                    tx(PodSelectorMsg::ClusterChanged(active));
                }
            });

            let row = gtk::BoxBuilder::new()
                .build();

            row.add(&gtk::LabelBuilder::new()
                .label("Cluster: ")
                .xalign(0.0)
                .width_request(100)
                .build()
            );
            row.pack_end(&cluster_selector, true, true, 0);
            col.add(&row);
        }

        let namespace_model = gtk::ListStore::new(&[glib::Type::String]);
        let namespace_selector = ComboBoxTextBuilder::new()
            .id_column(0)
            .model(&namespace_model)
            .build();
        {
            let tx = tx.clone();
            namespace_selector.connect_changed(move |combo|{
                if let Some(active) = combo.get_active_id() {
                    tx(PodSelectorMsg::NamespaceChanged(active.to_string()));
                }
            });
            let row = gtk::BoxBuilder::new()
                .build();

            row.add(&gtk::LabelBuilder::new()
                .label("Namespace: ")
                .xalign(0.0)
                .width_request(100)
                .build()
            );
            row.pack_end(&namespace_selector, true, true, 0);
            col.add(&row);
        }

        let scroll_wnd = ScrolledWindowBuilder::new()
            .expand(true)
            .build();

        scroll_wnd.add(&list);
        content.add(&scroll_wnd);

        let rules_view = HighlighterListView::new();
        rules_view.add_highlighters(get_default_highlighters());

        let sw = ScrolledWindowBuilder::new()
            .expand(true)
            .build();
        sw.add(rules_view.view());
        let lbl = gtk::LabelBuilder::new()
            .label("Rules")
            .halign(Align::Start)
            .build();

        add_css_with_name(&lbl, "rules", r##"
        #rules {
            font-size: 20px;
            padding-top: 10px;
            padding-bottom: 10px;
        }
        "##);

        content.add(&lbl);
        content.add(&sw);

        let log_separate_tab_checkbox = gtk::CheckButton::with_label("Logs in separate tab");
        {
            let tx = tx.clone();
            log_separate_tab_checkbox.connect_toggled(move|checkbox|{
                tx(PodSelectorMsg::ToggleSeparateTabs(checkbox.get_active()));
            });
            log_separate_tab_checkbox.set_active(true);
            content.add(&log_separate_tab_checkbox);
        }

        let include_replicas = gtk::CheckButton::with_label("Include replicas");
        {
            let tx = tx.clone();
            include_replicas.set_active(true);
            include_replicas.connect_toggled(move |checkbox| {
                tx(PodSelectorMsg::ToggleIncludeReplicas(checkbox.get_active()));
            });
            content.add(&include_replicas);
        }

        let since_val = gtk::SpinButton::new(
            Some(&gtk::Adjustment::new(4.0, 1.0, 721.0, 1.0, 1.0, 1.0))
            , 1.0, 0);
        {
            let tx = tx.clone();
            since_val.set_value(10.0);
            since_val.connect_changed(move |since_val| {
                let since = since_val.get_text().to_string();
                let since = since.parse::<u32>().unwrap_or(10);
                tx(PodSelectorMsg::SinceChanged(since));
            });
        }

        let since_row = BoxBuilder::new()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .margin_top(8)
            .build();

        since_row.add(&gtk::Label::new(Some("Since:")));
        since_row.add(&since_val);

        let unit_selector = gtk::ComboBoxText::new();
        {
            let tx = tx.clone();
            unit_selector.append(Some(UNIT_MINUTES), "Minutes");
            unit_selector.append(Some(UNIT_HOURS), "Hours");
            unit_selector.set_active(Some(0));
            unit_selector.connect_changed(move |combo| {
                let active_unit = combo.get_active_id().map(|s| s.to_string()).unwrap_or(UNIT_MINUTES.to_string());
                tx(PodSelectorMsg::SinceUnitChanged(active_unit));
            });
        }

        since_row.add(&unit_selector);
        content.add(&since_row);

        dlg.connect_delete_event(move |dlg, _| {
            dlg.hide();
            gtk::Inhibit(true)
        });


        let btn_row = gtk::BoxBuilder::new()
            .spacing(5)
            .margin_top(10)
            .build();
        content.add(&btn_row);

        let ok_btn = gtk::ButtonBuilder::new()
            .label("Ok")
            .width_request(80)
            .build();
        {
            let tx = tx.clone();
            ok_btn.connect_clicked(move|_|{
                tx(PodSelectorMsg::Ok);
            });
            btn_row.add(&ok_btn);
        }

        let close_btn = gtk::ButtonBuilder::new()
            .label("Close")
            .width_request(80)
            .build();
        {
            let tx = tx.clone();
            close_btn.connect_clicked(move|_|{
                tx(PodSelectorMsg::Close);
            });
            btn_row.add(&close_btn);
        }

        Self {
            dlg,
            kube_client: None,
            pods_model: service_model,
            namespace_model,
            separate_tabs: true,
            since_multiplier: 60,
            since: 10,
            rules_view,
            namespace_selector,
            cluster_selector,
            selected_namespace: None,
            selected_cluster: None,
            include_replicas: true,
            msg
        }
    }

    fn load_clusters(&mut self) {
        self.cluster_selector.remove_all();
        if let Ok(cfg) = KubeConfig::load_default() {
            for context in &cfg.contexts {
                self.cluster_selector.append(Some(&context.name), &context.name);
            }

            let cluster_to_select = self.selected_cluster.as_ref().unwrap_or(&cfg.current_context);
            self.cluster_selector.set_active_id(Some(&cluster_to_select));
        }
    }

    fn load_pods(&mut self) {
        match self.get_pods() {
            Ok(pods) => {
                self.pods_model.clear();
                let mut duplicates = HashSet::new();
                for pod in pods.into_iter() {
                    let name = if self.include_replicas {
                        pod.spec.containers.first().map(|c| c.name.clone()).unwrap_or_default()
                    } else {
                        pod.metadata.name.unwrap_or_default()
                    };

                    if !duplicates.contains(&name) {
                        self.pods_model.insert_with_values(None, None, &[0, 1], &[&name, &false]);
                        duplicates.insert(name);
                    }
                }
            }
            Err(e) => {
                error!("Could not get pods: {}", e);
            }
        }
    }

    fn get_namespaces(&self) -> Result<Vec<Namespace>, Box<dyn Error>> {
        let client= self.kube_client.as_ref().ok_or(anyhow::Error::msg("No k8s client"))?.clone();
        glib::MainContext::default().block_on(async move  {
            client.namespaces().await
        })
    }

    fn get_pods(&self) -> Result<Vec<Pod>, Box<dyn Error>> {
        let client = self.kube_client.as_ref().ok_or(anyhow::Error::msg("No k8s client"))?.clone();
        let default_namespace = "default".to_string();
        let namespace = self.selected_namespace.as_ref().unwrap_or(&default_namespace);
        info!("Loading pods for namespace: {}", namespace);
        glib::MainContext::default().block_on(async move  {
            client.pods(namespace).await
        })
    }

    pub fn update(&mut self, msg: PodSelectorMsg) {
        match msg {
            PodSelectorMsg::Show => {
                self.load_clusters();
                self.dlg.show_all();
            }
            PodSelectorMsg::Close => {
                self.dlg.hide();
            }
            PodSelectorMsg::Ok => {
                self.dlg.hide();
                let mut models = vec![];
                if let Some(iter) = self.pods_model.get_iter_first() {
                    if let Some((service, active)) = map_model(&self.pods_model, &iter) {
                        if active {
                            models.push(service);
                        }
                    }
                    while self.pods_model.iter_next(&iter) {
                        if let Some((service, active)) = map_model(&self.pods_model, &iter) {
                            if active {
                                models.push(service);
                            }
                        }
                    }
                }

                let highlighters = self.rules_view.get_highlighter().unwrap_or_default();
                let since = self.since * self.since_multiplier;

                if let (Some(selected_cluster), Some(selected_namespace)) = (self.selected_cluster.as_ref(), self.selected_namespace.as_ref()) {
                    if self.separate_tabs {
                        for model in models {
                            self.msg.send(CreateLogView::with_rules(LogViewData::Kube(CreateKubeLogData {
                                pods: vec![model],
                                cluster: selected_cluster.clone(),
                                namespace: selected_namespace.clone(),
                                since,
                            }), highlighters.clone())).expect("Could not send create tab msg");
                        }
                    } else {
                        self.msg.send(CreateLogView::with_rules(LogViewData::Kube(CreateKubeLogData {
                            pods: models,
                            cluster: selected_cluster.clone(),
                            namespace: selected_namespace.clone(),
                            since,
                        }), highlighters)).expect("Could not send create tab msg");
                    }
                }
            },
            PodSelectorMsg::ToggleIncludeReplicas(include) => {
                self.include_replicas = include;
                self.load_pods();
            }
            PodSelectorMsg::ToggleSeparateTabs(separate) => {
                self.separate_tabs = separate;
            }
            PodSelectorMsg::SinceUnitChanged(active_unit) => {
                self.since_multiplier = if active_unit.as_str() == UNIT_MINUTES {
                    60
                } else {
                    60 * 60
                };
            }
            PodSelectorMsg::SinceChanged(since) => {
                self.since = since;
            }
            PodSelectorMsg::ClusterChanged(cluster) => {
                if self.selected_cluster.as_ref() != Some(&cluster) {
                    self.selected_cluster.replace(cluster.clone());
                    match KubeConfig::load_default()
                        .and_then(|cfg| cfg.context(&cluster))
                        .and_then(|ctx| KubeClient::with_options(&ctx, Some(ClientOptions {timeout: Some(Duration::from_secs(5))}))) {
                        Ok(kube_client) => {
                            self.kube_client.replace(kube_client);
                            self.namespace_model.clear();

                            match self.get_namespaces() {
                                Ok(namespaces) => {
                                    for namespace in namespaces {
                                        let name = namespace.metadata.name.unwrap();
                                        self.namespace_model.insert_with_values(None, &[0], &[&name]);
                                    }
                                    self.namespace_selector.set_active_id(Some("default"));
                                }
                                Err(e) => {
                                    error!("Could not get namespaces for {}: {}", cluster, e);
                                    show_error_msg(&e.to_string());
                                }
                            }
                        }
                        Err(e) => {
                            error!("Could not get k8s client: {}", e);
                            show_error_msg(&e.to_string());
                        }
                    }
                }
            }
            PodSelectorMsg::NamespaceChanged(namespace) => {
                self.selected_namespace.replace(namespace);
                self.load_pods();
            }
        }
    }
}