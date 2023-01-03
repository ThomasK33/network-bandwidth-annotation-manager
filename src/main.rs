mod quantity_parser;

use std::{net::SocketAddr, path::PathBuf};

use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use color_eyre::Result;
use json_patch::{AddOperation, CopyOperation, PatchOperation, RemoveOperation};
use kube::{
    core::{
        admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
        DynamicObject,
    },
    Resource, ResourceExt,
};
use tracing::{error, info, warn};

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
            "/override",
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

    if let Some(config) = config {
        tracing::debug!("tls listening on {}", &cli.addr);
        axum_server::bind_rustls(cli.addr, config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        tracing::debug!("listening on {}", &cli.addr);
        axum::Server::bind(&cli.addr)
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
                        .map(|bandwidth| quantity_parser::parse(bandwidth)),
                    requests
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(|bandwidth| quantity_parser::parse(bandwidth)),
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
                        .map(|bandwidth| quantity_parser::parse(bandwidth)),
                    limits
                        .get(ingress_bandwidth_resource_key)
                        .and_then(|bandwidth| bandwidth.as_str())
                        .map(|bandwidth| quantity_parser::parse(bandwidth)),
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

fn escape_json_pointer(key: &str) -> String {
    key.replace("~", "~0").replace("/", "~1")
}

fn convert_filter(filter: log::LevelFilter) -> tracing_subscriber::filter::LevelFilter {
    match filter {
        log::LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        log::LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
        log::LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
        log::LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
        log::LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
        log::LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
    }
}
