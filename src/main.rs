use std::{net::SocketAddr, path::PathBuf};

use anyhow::Result;
use axum::{http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
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
    tls_cert_file: Option<PathBuf>,
    /// Path to PEM encoded TLS private key file
    #[clap(long, env)]
    tls_key_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(convert_filter(cli.verbose.log_level_filter()))
        .init();

    let app = Router::new().route("/mutate", post(mutate_handler));

    let config: Option<RustlsConfig> = if let Some(tls_cert_file) = cli.tls_cert_file {
        if let Some(tls_key_file) = cli.tls_key_file {
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
    }
}

// A general /mutate handler, handling errors from the underlying business logic
async fn mutate_handler(Json(body): Json<AdmissionReview<DynamicObject>>) -> impl IntoResponse {
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

        res = match mutate(res.clone(), &obj) {
            Ok(res) => {
                // TODO: Remove those verbose logs
                info!("accepted: {:?} on Foo {}", req.operation, name);

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
fn mutate(res: AdmissionResponse, obj: &DynamicObject) -> Result<AdmissionResponse> {
    if let Some(containers) = obj.data.get("spec").and_then(|spec| {
        spec.get("containers")
            .and_then(|containers| containers.as_array())
    }) {
        for container in containers {
            let Some(resources) = container.get("resources") else { continue };
            let Some(limits) = resources.get("limits") else { continue; };

            // TODO: Add logic adding up all the ingress-bandwidth and egress-bandwidth
        }
    }

    // TODO: Add annotations to pod if limits exist

	// TODO: Remove this once the above are completed
    // If the resource doesn't contain "admission", we add it to the resource.
    if !obj.labels().contains_key("admission") {
        let mut patches = Vec::new();

        // Ensure labels exist before adding a key to it
        if obj.meta().labels.is_none() {
            patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: "/metadata/labels".into(),
                value: serde_json::json!({}),
            }));
        }
        // Add our label
        patches.push(json_patch::PatchOperation::Add(json_patch::AddOperation {
            path: "/metadata/labels/admission".into(),
            value: serde_json::Value::String("modified-by-admission-controller".into()),
        }));

        Ok(res.with_patch(json_patch::Patch(patches))?)
    } else {
        Ok(res)
    }
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
