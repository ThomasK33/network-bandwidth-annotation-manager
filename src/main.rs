#![forbid(unsafe_code)]
mod quantity_parser;
mod utilrs;

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use color_eyre::Result;
use futures::TryStreamExt;
use json_patch::{AddOperation, CopyOperation, PatchOperation, RemoveOperation};
use k8s_openapi::api::core::v1::Namespace;
use kube::{
    api::ListParams,
    core::{
        admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
        DynamicObject, ObjectMeta,
    },
    runtime::{watcher, WatchStreamExt},
    Api, Client, Resource, ResourceExt,
};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};
use utilrs::{convert_filter, escape_json_pointer};

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity<InfoLevel>,

    /// Socket listen address
    #[clap(long = "listen", short = 'l', env, default_value_t = SocketAddr::from(([127, 0, 0, 1], 3000)))]
    addr: SocketAddr,
    /// Path to PEM encoded TLS cert file
    #[clap(long, env)]
    tls_cert: Option<PathBuf>,
    /// Path to PEM encoded TLS private key file
    #[clap(long, env)]
    tls_key: Option<PathBuf>,

    /// Egress bandwidth resource key name
    #[clap(long, env, default_value_t = { "networking.k8s.io/egress-bandwidth".to_owned() })]
    egress_bandwidth_resource_key: String,
    /// Ingress bandwidth resource key name
    #[clap(long, env, default_value_t = { "networking.k8s.io/ingress-bandwidth".to_owned() })]
    ingress_bandwidth_resource_key: String,
}

enum Mode {
    Annotate,
    Strip,
    Overwrite,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(convert_filter(cli.verbose.log_level_filter()))
        .init();

    // TODO: Add Kubernetes client fetching and caching all namespaces into a Arc
    // Mutex HashMap. This is necessary so that later in the scheduler override the
    // scheduler name can be derived from the label on the namespace
    let namespaces: Arc<Mutex<HashMap<String, ObjectMeta>>> = Arc::new(Mutex::new(HashMap::new()));

    // FIXME: Do not drop watcher error silently, instead start a new watcher task
    let _: JoinHandle<Result<()>> = tokio::spawn({
        let namespaces = namespaces.clone();

        async move {
            let client = Client::try_default().await?;

            watcher(Api::<Namespace>::all(client), ListParams::default())
                .applied_objects()
                .try_for_each(|namespace| {
                    let namespaces = namespaces.clone();

                    async move {
                        tracing::info!("namespace: {namespace:?}");

                        let name = namespace.name_any();
                        let meta = namespace.meta();

                        if let Ok(mut namespaces) = namespaces.lock() {
                            namespaces.insert(name, meta.clone());
                        }

                        Ok(())
                    }
                })
                .await?;

            Ok(())
        }
    });

    let app = Router::new()
        .route(
            "/mutate",
            post({
                let egress_bandwidth_resource_key = cli.egress_bandwidth_resource_key.clone();
                let ingress_bandwidth_resource_key = cli.ingress_bandwidth_resource_key.clone();

                move |body| {
                    mutate_handler(
                        body,
                        egress_bandwidth_resource_key,
                        ingress_bandwidth_resource_key,
                        Mode::Annotate,
                    )
                }
            }),
        )
        .route(
            "/strip",
            post({
                let egress_bandwidth_resource_key = cli.egress_bandwidth_resource_key.clone();
                let ingress_bandwidth_resource_key = cli.ingress_bandwidth_resource_key.clone();

                move |body| {
                    mutate_handler(
                        body,
                        egress_bandwidth_resource_key,
                        ingress_bandwidth_resource_key,
                        Mode::Strip,
                    )
                }
            }),
        )
        .route(
            "/overwrite",
            post({
                let egress_bandwidth_resource_key = cli.egress_bandwidth_resource_key.clone();
                let ingress_bandwidth_resource_key = cli.ingress_bandwidth_resource_key.clone();

                move |body| {
                    mutate_handler(
                        body,
                        egress_bandwidth_resource_key,
                        ingress_bandwidth_resource_key,
                        Mode::Overwrite,
                    )
                }
            }),
        )
        .route(
            "/namespaces",
            get({
                tracing::debug!("namespaces: {:?}", namespaces);

                || async move { StatusCode::OK }
            }),
        );

    let config: Option<RustlsConfig> = if let Some(tls_cert_file) = cli.tls_cert {
        if let Some(tls_key_file) = cli.tls_key {
            // TODO: Implement certificate rotation logic
            match RustlsConfig::from_pem_file(tls_cert_file, tls_key_file).await {
                Ok(config) => Some(config),
                Err(err) => {
                    error!("Could not build rustls config: {err:?}");

                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    // TODO: Handle graceful shutdown

    if let Some(config) = config {
        tracing::debug!("tls listening on {}", &cli.addr);
        axum_server::bind_rustls(cli.addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        tracing::debug!("listening on {}", &cli.addr);
        axum_server::bind(cli.addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    };

    Ok(())
}

// A general /mutate handler, handling errors from the underlying business logic
async fn mutate_handler(
    Json(body): Json<AdmissionReview<DynamicObject>>,
    egress_bandwidth_resource_key: String,
    ingress_bandwidth_resource_key: String,
    mode: Mode,
) -> impl IntoResponse {
    // Parse incoming webhook AdmissionRequest first
    let req: AdmissionRequest<_> = match body.try_into() {
        Ok(req) => req,
        Err(err) => {
            error!("invalid request: {}", err.to_string());
            return (
                StatusCode::BAD_REQUEST,
                Json(AdmissionResponse::invalid(err.to_string()).into_review()),
            );
        }
    };

    // Then construct a AdmissionResponse
    let mut res = AdmissionResponse::from(&req);

    // req.Object always exists for us, but could be None if extending to DELETE events
    if let Some(obj) = req.object {
        let name = obj.name_any(); // apiserver may not have generated a name yet

        res = match mutate(
            res.clone(),
            &obj,
            &egress_bandwidth_resource_key,
            &ingress_bandwidth_resource_key,
            mode,
        ) {
            Ok(res) => {
                // TODO: Remove those verbose logs
                info!("accepted: {:?} on pod {}", req.operation, name);

                res
            }
            Err(err) => {
                warn!("denied: {:?} on {} ({})", req.operation, name, err);
                res.deny(err.to_string())
            }
        };
    };

    // Wrap the AdmissionResponse wrapped in an AdmissionReview
    (StatusCode::OK, Json(res.into_review()))
}

// The main handler and core business logic, failures here implies rejected applies
fn mutate(
    res: AdmissionResponse,
    obj: &DynamicObject,
    egress_bandwidth_resource_key: &str,
    ingress_bandwidth_resource_key: &str,
    mode: Mode,
) -> Result<AdmissionResponse> {
    let mut patches = Vec::new();

    // If the resource doesn't contain "admission", we add it to the resource.
    if !obj.annotations().contains_key("nba-admission") {
        // Ensure annotations exist before adding a key to it
        if obj.meta().annotations.is_none() {
            patches.push(PatchOperation::Add(AddOperation {
                path: "/metadata/annotations".into(),
                value: serde_json::json!({}),
            }));
        }

        // Add our annotation
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/nba-admission".into(),
            value: serde_json::Value::String("true".into()),
        }));
    };

    let mut egress_request: Option<f64> = None;
    let mut ingress_request: Option<f64> = None;
    let mut egress_limit: Option<f64> = None;
    let mut ingress_limit: Option<f64> = None;

    if let Some(containers) = obj.data.get("spec").and_then(|spec| {
        spec.get("containers")
            .and_then(|containers| containers.as_array())
    }) {
        for (index, container) in containers.iter().enumerate() {
            let Some(resources) = container.get("resources") else { continue };

            // -- Get egress and ingress requests --
            let requests = resources.get("requests").map(|requests| {
                (
                    requests
                        .get(egress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity_parser::parse),
                    requests
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity_parser::parse),
                )
            });

            if let Some((egress, ingress)) = &requests {
                if let Some(Ok(egress)) = egress {
                    if egress_request.is_none() {
                        egress_request = Some(0.0);
                    }

                    if let Some(egress_request) = egress_request.as_mut() {
                        *egress_request += egress;
                    }
                }

                if let Some(Ok(ingress)) = ingress {
                    if ingress_request.is_none() {
                        ingress_request = Some(0.0);
                    }

                    if let Some(ingress_request) = ingress_request.as_mut() {
                        *ingress_request += ingress;
                    }
                }
            }

            // -- Get egress and ingress limits --
            let limits = resources.get("limits").map(|limits| {
                (
                    limits
                        .get(egress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity_parser::parse),
                    limits
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(quantity_parser::parse),
                )
            });

            if let Some((egress, ingress)) = &limits {
                if let Some(Ok(egress)) = egress {
                    if egress_limit.is_none() {
                        egress_limit = Some(0.0);
                    }

                    if let Some(egress_limit) = egress_limit.as_mut() {
                        *egress_limit += egress;
                    }
                }

                if let Some(Ok(ingress)) = ingress {
                    if ingress_limit.is_none() {
                        ingress_limit = Some(0.0);
                    }

                    if let Some(ingress_limit) = ingress_limit.as_mut() {
                        *ingress_limit += ingress;
                    }
                }
            }

            // -- Mutation modes --
            match mode {
                Mode::Annotate => {
                    // In annotate mode, no further operations have to be performed on the Kubernetes object
                    // thus it's a noop
                }
                // Strip custom bandwidth resource requests and limits if strip = true
                Mode::Strip => {
                    if let Some((egress, ingress)) = requests {
                        if let Some(Ok(_)) = egress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        if let Some(Ok(_)) = ingress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }

                    if let Some((egress, ingress)) = limits {
                        if let Some(Ok(_)) = egress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        if let Some(Ok(_)) = ingress {
                            patches.push(PatchOperation::Remove(RemoveOperation {
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }
                }
                Mode::Overwrite => {
                    // Check if current container has both, requests and limits set
                    if let (Some(limits), Some(requests)) = (limits, requests) {
                        // Check if current container has egress bandwidth defined in both requests and limits set
                        if let (Some(Ok(_)), Some(Ok(_))) = (limits.0, requests.0) {
                            patches.push(PatchOperation::Copy(CopyOperation {
                                from: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(egress_bandwidth_resource_key)
                                ),
                            }));
                        }

                        // Check if current container has ingress bandwidth defined in both requests and limits set
                        if let (Some(Ok(_)), Some(Ok(_))) = (limits.1, requests.1) {
                            patches.push(PatchOperation::Copy(CopyOperation {
                                from: format!(
                                    "/spec/containers/{index}/resources/requests/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                                path: format!(
                                    "/spec/containers/{index}/resources/limits/{}",
                                    escape_json_pointer(ingress_bandwidth_resource_key)
                                ),
                            }));
                        }
                    }
                }
            };
        }
    }

    // Add request annotations for use-cases with dedicated schedulers
    if let Some(egress_request) = egress_request {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1egress-request".into(),
            value: serde_json::Value::String(egress_request.to_string()),
        }));
    }
    if let Some(ingress_request) = ingress_request {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1ingress-request".into(),
            value: serde_json::Value::String(ingress_request.to_string()),
        }));
    }

    // Add annotations to pod if limits exist
    if let Some(egress_limit) = egress_limit {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1egress-bandwidth".into(),
            value: serde_json::Value::String(egress_limit.to_string()),
        }));
    }
    if let Some(ingress_limit) = ingress_limit {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/metadata/annotations/kubernetes.io~1ingress-bandwidth".into(),
            value: serde_json::Value::String(ingress_limit.to_string()),
        }));
    }

    Ok(if !patches.is_empty() {
        res.with_patch(json_patch::Patch(patches))?
    } else {
        res
    })
}

fn scheduler_mutate(res: AdmissionResponse, obj: &DynamicObject) -> Result<AdmissionResponse> {
    let mut patches = Vec::new();

    // Check if the pod has a scheduler label
    if let Some(default_scheduler) = obj.labels().get("nbam-default-scheduler") {
        patches.push(PatchOperation::Add(AddOperation {
            path: "/spec/schedulerName".into(),
            value: serde_json::Value::String(default_scheduler.to_owned()),
        }));
    } else {
        // Otherwise this function was called by a namespace with a label set thus we
        // need to fetch the namespace details and get the label from there
    }

    Ok(if !patches.is_empty() {
        res.with_patch(json_patch::Patch(patches))?
    } else {
        res
    })
}

// TODO: Add unit tests
