use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use stream_cancel::{StreamExt as StreamCancelStreamExt, Trigger, Tripwire};
use tokio_stream::wrappers::ReceiverStream;
use crate::k8s_client::{KubeClient, LogOptions, KubeConfig};
use crate::pod_list_view::PodViewData;
use crate::tokio;


static LOG_LINE_PATTERN: Lazy<Regex> = Lazy::new(||{
    Regex::new(r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}.\d{9}Z)\s(?P<data>.*\n)").expect("Invalid regex")
});

#[derive(Clone)]
pub struct LogData {
    pub text: String,
    pub pod: String,
    pub container: String,
    pub timestamp: DateTime<Utc>,
}

pub async fn log_stream(k8s_client: &KubeClient, namespace: &str, pods: Vec<PodViewData>, since_seconds: u32) -> (impl Stream<Item = LogData>, Trigger) {
    let (tx, rx) = tokio::sync::mpsc::channel::<LogData>(1000);
    let (trigger, tripwire) = Tripwire::new();
    for pod in pods {
        for container in pod.containers() {
            let tripwire = tripwire.clone();
            let k8s_client = k8s_client.clone();
            let tx = tx.clone();
            let namespace = namespace.to_string();
            let pod = pod.clone();

            tokio::task::spawn(async move {
                log::info!("Start tail for {} ({})", pod.name, container);
                let res = k8s_client.logs(&namespace, &pod.name, Some(&container), Some(LogOptions {
                    since_seconds: Some(since_seconds),
                    follow: Some(true),
                })).await.unwrap();

                let mut res = res.take_until_if(tripwire);
                let mut buffer = Vec::new();
                while let Some(Ok(bytes)) = res.next().await {
                    buffer.append(&mut bytes.to_vec());
                    let data = String::from_utf8_lossy(&buffer).to_string();
                    if let Some(ma) = LOG_LINE_PATTERN.captures(&data) {
                        if let Some(timestamp) = ma.name("timestamp")
                            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts.as_str()).ok()
                            .and_then(|dt| Some(dt.with_timezone(&Utc))))
                        {
                            let log_data = ma.name("data").unwrap().as_str().to_string();
                            if let Err(e ) = tx.send(LogData { pod: pod.name.clone(), container: container.clone(), text: log_data, timestamp }).await {
                                log::error!("Failed to send data: {}", e);
                            }
                        }
                        else {
                            log::error!("Invalid log data data without timestamp")
                        }
                        buffer.clear();
                    }
                }
                log::info!("Stopped tail for: {} ({})", pod.name, container);
            });
        }
    }

    (ReceiverStream::new(rx), trigger)
}

pub fn k8s_client(path: &str, ctx: &str) -> KubeClient {
    let cfg = KubeConfig::load(path).unwrap();
    let ctx = cfg.context(ctx).unwrap();
    KubeClient::new(&ctx).unwrap()
}

pub fn k8s_client_with_timeout(path: &str, ctx: &str) -> KubeClient {
    let cfg = KubeConfig::load(path).unwrap();
    let ctx = cfg.context(ctx).unwrap();
    KubeClient::with_timeout(&ctx).unwrap()
}
