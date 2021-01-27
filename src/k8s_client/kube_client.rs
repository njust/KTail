use reqwest::{self, header, ClientBuilder};
use openssl::pkcs12::Pkcs12;
use url::Url;
use crate::k8s_client::{Result, ClusterContext};
use serde::{Deserialize, Serialize};
use k8s_openapi::api::core::v1::{PodSpec, PodStatus, NamespaceSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use tokio::stream::Stream;

// Problems with empty password on MacOs
const PKCS12_PWD: &'static str = "a8c51701-bc96-44a4-a3bc-9b6034d1f8bd";

#[derive(Clone)]
pub struct KubeClient {
    pub(crate) client: reqwest::Client,
    pub(crate) base_url: Url,
}

pub struct LogOptions {
    pub follow: Option<bool>,
    pub since_seconds: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Pod {
    pub spec: PodSpec,
    pub metadata: ObjectMeta,
    pub status: Option<PodStatus>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Namespace {
    pub spec: NamespaceSpec,
    pub metadata: ObjectMeta,
    pub status: Option<PodStatus>,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListResult<T> {
    kind: String,
    api_version: String,
    pub items: Vec<T>,
}

pub struct ClientOptions {
    pub timeout: Option<std::time::Duration>,
}

impl KubeClient {
    pub fn new(context: &ClusterContext) -> Result<KubeClient> {
        KubeClient::with_options(context, None)
    }
    pub fn with_options(context: &ClusterContext, options: Option<ClientOptions>) -> Result<KubeClient> {
        let auth_info = &context.user;
        let cluster = &context.cluster;

        let mut client = ClientBuilder::new();
        if let Some(options) = options {
            if let Some(timeout) = options.timeout {
                client = client.timeout(timeout);
            }
        }

        let mut headers = header::HeaderMap::new();

        let client = if let Some(ca_cert) = cluster.ca_cert() {
            let req_ca_cert = reqwest::Certificate::from_der(&ca_cert.to_der().unwrap()).unwrap();
            client.add_root_certificate(req_ca_cert)
        } else { client };

        let client = if auth_info.client_certificate().is_some() && auth_info.client_key().is_some() {
            let crt = auth_info.client_certificate().unwrap();
            let key = auth_info.client_key().unwrap();
            let pkcs_cert = Pkcs12::builder().build(PKCS12_PWD, "admin", &key, &crt)?;
            let req_pkcs_cert = reqwest::Identity::from_pkcs12_der(&pkcs_cert.to_der().expect("Could not convert pkcs to der cert"), PKCS12_PWD).unwrap();
            client.identity(req_pkcs_cert)
        } else { client };

        // if let (Some(username), Some(password)) = (auth_info.username, auth_info.password) {
        //     headers.typed_insert(headers_ext::Authorization::basic(
        //         &username, &password
        //     ));
        // } else if let Some(token) = auth_info.token {
        //     headers.typed_insert(headers_ext::Authorization::bearer(&token)
        //                          .map_err(|_| Error::from("Invalid bearer token"))?);
        // }

        if let Some(token) = &auth_info.token {
            headers.insert("Authorization", format!("Bearer {}", token).parse().unwrap());
        }

        let client = client.default_headers(headers)
            .build()?;

        Ok(KubeClient { client, base_url: cluster.server.clone() })
    }

    pub async fn pods(&self, namespace: &str) -> Result<Vec<Pod>> {
        let url = format!("{}api/v1/namespaces/{}/pods", self.base_url, namespace);
        let response = self.client.get(&url)
            .send().await?;
        Ok(response.json::<ListResult<Pod>>().await.map(|r|r.items)?)
    }

    pub async fn logs(&self, namespace: &str, pod: &str, options: Option<LogOptions>) -> Result<impl Stream<Item = reqwest::Result<bytes::Bytes>>> {
        let url = format!("{}api/v1/namespaces/{}/pods/{}/log", self.base_url, namespace, pod);
        let mut request = self.client.get(&url);
        let response = if let Some(opt) = options {
            if opt.follow.is_some() && opt.follow.unwrap() {
                request = request.query(&[("follow", "true")])
            }
            if let Some(since) = opt.since_seconds {
                request = request.query(&[("sinceSeconds", since.to_string())])
            }
            request
        }else {
            request
        }.send().await?.bytes_stream();

        Ok(response)
    }

    pub async fn namespaces(&self) -> Result<Vec<Namespace>> {
        let url = format!("{}api/v1/namespaces", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.json::<ListResult<Namespace>>().await.map(|r|r.items)?)
    }
}

#[test]
pub fn test_load_namespaces() -> crate::Result<()> {
    use crate::k8s_client::{Result, ClusterContext, KubeConfig};

    let mut rt = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build().unwrap();
    rt.block_on(async move  {
        let cfg = KubeConfig::load_default().unwrap();
        let cx_names: Vec<&String> = cfg.contexts.iter().map(|c|&c.name).collect();
        println!("{:?}", cx_names);
        let ctx = cfg.context("DEVCluster").unwrap();

        let client = KubeClient::new(&ctx).unwrap();
        match client.pods("kube-system").await {
            Ok(pods) => {
                println!("{:?}", pods);
            }
            Err(e) => {
                println!("{:?}", e);
            }
        }

    });

    Ok(())
}

