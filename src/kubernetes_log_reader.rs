use crate::model::{CreateKubeLogData};
use std::collections::HashMap;
use std::error::Error;
use crate::k8s_client::{KubeClient, LogOptions, KubeConfig};
use stream_cancel::{Valved, Trigger};
use tokio::sync::oneshot::Sender;
use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;
use log::{info, error, debug};
use crate::model::{LogReader, LogState};

pub struct KubernetesLogReader {
    options: CreateKubeLogData,
    is_initialized: bool,
    is_stopping: bool,
    data_rx: Option<Receiver<KubernetesLogReaderMsg>>,
    data_tx: Option<tokio::sync::mpsc::Sender<KubernetesLogReaderMsg>>,
    streams: HashMap<String, (Sender<Trigger>, Trigger)>
}

pub enum KubernetesLogReaderMsg {
    Data(Vec<u8>),
    ReInit(PodContainerData),
}

#[derive(Clone)]
pub struct PodContainerData {
    pod: String,
    container: String,
    key: String,
}

impl PodContainerData {
    fn new(pod: String, container: String) -> Self {
        Self {
            key: format!("{}#{}", pod, container),
            pod,
            container,
        }
    }
    fn key(&self) -> &str {
        self.key.as_str()
    }
    fn prefix(&self) -> String {
        let pod_id = self.pod.split("-").last().unwrap_or(&self.pod);
        format!("{}-{}", &self.container, pod_id)
    }
}

#[async_trait]
impl LogReader for KubernetesLogReader {
    async fn read(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut r = vec![];
        loop {
            if let Some(rx) = self.data_rx.as_mut() {
                if let Ok(rc) = rx.try_recv() {
                    match rc {
                        KubernetesLogReaderMsg::Data(mut data) => {
                            if data.len() > 0 {
                                r.append(&mut data)
                            }else {
                                break;
                            }
                        }
                        KubernetesLogReaderMsg::ReInit(pod) => {
                            self.is_initialized = false;
                            self.streams.remove(pod.key());
                            break;
                        }
                    }
                }else {
                    break;
                }
            }
        }
        Ok(r)
    }

    async fn init(&mut self) {
        use tokio::stream::StreamExt;
        if self.is_initialized || self.is_stopping {
            return;
        }

        let client = match KubeConfig::load_default()
            .and_then(|config| config.context(&self.options.cluster))
            .and_then(|ctx| KubeClient::new(&ctx)) {
            Ok(client) => {
                self.is_initialized = true;
                client
            },
            Err(e) => {
                error!("Could not init k8s client: {}", e);
                return;
            }
        };

        let mut pod_list = vec![];
        if let Ok(pods) = client.pods(&self.options.namespace).await {
            for pod in pods {
                if let Some(name) = pod.metadata.name {
                    for pod_name in &self.options.pods {
                        if name.starts_with(pod_name) {
                            if let Some(container_status) = pod.status.as_ref().and_then(|s|s.container_statuses.as_ref()).and_then(|cs|cs.first()) {
                                if container_status.ready {
                                    for container in pod.spec.containers {
                                        pod_list.push(PodContainerData::new(name.clone(), container.name.clone()));
                                    }
                                }else {
                                    self.is_initialized = false;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        let prefix_log_entries = pod_list.len() > 1;
        let mut max_prefix_len = 0;
        for p in &pod_list {
            let prefix_len = p.prefix().len();
            if prefix_len > max_prefix_len {
                max_prefix_len = prefix_len;
            }
        }
        max_prefix_len = max_prefix_len + 3;
        for pod in pod_list {
            if self.streams.contains_key(pod.key()) {
                // info!("Skipping initiate stream for pod '{}'", pod);
                continue;
            }

            if let Ok(log_stream) = client.logs(&self.options.namespace, &pod.pod, &pod.container, Some(
                LogOptions {
                    follow: Some(true),
                    since_seconds: Some(self.options.since),
                }
            )).await {
                let (exit_tx, _exit_rx) = tokio::sync::oneshot::channel::<stream_cancel::Trigger>();
                let (exit, mut inc) = Valved::new(log_stream);
                self.streams.insert(pod.key().to_string(), (exit_tx, exit));
                let mut tx = self.data_tx.clone().unwrap();
                let pod_data = pod.clone();
                tokio::spawn(async move {
                    info!("Stream for pod '{}' started", pod_data.key());
                    let prefix = pod_data.prefix();
                    let spaces = max_prefix_len - prefix.len();
                    let space = (0..spaces).map(|_|"").collect::<Vec<&str>>().join(" ");
                    while let Some(Ok(res)) = inc.next().await {
                        let data = if prefix_log_entries {
                            let mut data = format!("[{}]{}", prefix, space).into_bytes();
                            data.append(&mut res.to_vec());
                            data
                        }else {
                            res.to_vec()
                        };

                        if let Err(e) = tx.send(KubernetesLogReaderMsg::Data(data)).await {
                            error!("Failed to send stream data for pod '{}': {}", pod_data.key(), e);
                        }
                    }
                    info!("Stream for pod '{}' ended", pod_data.key());
                    if let Err(e) = tx.send(KubernetesLogReaderMsg::ReInit(pod_data.clone())).await {
                        debug!("Could not send kubernetes re init msg: {}", e);
                    }
                });
            }
        }
    }

    fn check_changes(&mut self) -> LogState {
        LogState::Ok
    }

    fn stop(&mut self) {
        self.is_stopping = true;
        let pods = self.streams.keys().map(|s|s.clone()).collect::<Vec<String>>();
        for p in pods {
            if let Some((sender, trigger)) = self.streams.remove(&p) {
                if let Err(e) = sender.send(trigger) {
                    debug!("Could not send exit trigger: {:?}", e);
                }
            }
        }
    }
}

impl KubernetesLogReader {
    pub fn new(data: CreateKubeLogData) -> Self {
        let (data_tx, data_rx) = tokio::sync::mpsc::channel::<KubernetesLogReaderMsg>(10000);
        let instance = Self {
            data_rx: Some(data_rx),
            data_tx: Some(data_tx),
            options: data,
            is_initialized: false,
            is_stopping: false,
            streams: HashMap::new(),
        };
        instance
    }
}