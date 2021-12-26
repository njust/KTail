use futures::{Stream, StreamExt};
use stream_cancel::{StreamExt as StreamCancelStreamExt, Trigger, Tripwire};
use tokio_stream::wrappers::ReceiverStream;
use crate::k8s_client::{KubeClient, LogOptions, KubeConfig};
use crate::tokio;

pub async fn log_stream(k8s_client: &KubeClient, namespace: &str, pods: &Vec<String>, since_seconds: u32) -> (impl Stream<Item = Vec<u8>>, Trigger) {
    let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1000);
    let (trigger, tripwire) = Tripwire::new();
    for pod in pods {
        let tripwire = tripwire.clone();
        let k8s_client = k8s_client.clone();
        let tx = tx.clone();

        let namespace = namespace.to_string();
        let pod = pod.clone();
        tokio::task::spawn(async move {
            let res = k8s_client.logs(&namespace, &pod, None, Some(LogOptions {
                since_seconds: Some(since_seconds),
                follow: Some(true),
            })).await.unwrap();

            let mut res = res.take_until_if(tripwire);
            let prefix = format!("{}   ", pod).into_bytes();
            while let Some(Ok(bytes)) = res.next().await {
                let mut r = bytes.to_vec();
                let mut data = prefix.clone();
                data.append(&mut r);
                if let Err(e ) = tx.send(data).await {
                    eprintln!("Failed to send data: {}", e);
                }
            }
            println!("Stopped tail for: {}", pod);
        });
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
