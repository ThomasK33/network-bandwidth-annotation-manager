#![forbid(unsafe_code)]
mod controller;
mod mutate;
mod utils;

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use axum::{routing::post, Router};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use clap_verbosity_flag::InfoLevel;
use color_eyre::Result;
use kube::core::ObjectMeta;

use mutate::{BandwidthMode, BandwidthProps, Mode};
use tracing::error;
use utils::convert_filter;

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

pub(crate) type NamespaceCache = Arc<Mutex<HashMap<String, ObjectMeta>>>;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_max_level(convert_filter(cli.verbose.log_level_filter()))
        .init();

    let namespaces: NamespaceCache = Arc::new(Mutex::new(HashMap::new()));

    controller::run(namespaces.clone());

    let app = Router::new()
        .route(
            "/annotate",
            post({
                let egress_bandwidth_resource_key = cli.egress_bandwidth_resource_key.clone();
                let ingress_bandwidth_resource_key = cli.ingress_bandwidth_resource_key.clone();

                move |body| {
                    mutate::handler(
                        body,
                        Mode::Bandwidth(BandwidthProps {
                            egress_bandwidth_resource_key,
                            ingress_bandwidth_resource_key,
                            mode: BandwidthMode::Annotate,
                        }),
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
                    mutate::handler(
                        body,
                        Mode::Bandwidth(BandwidthProps {
                            egress_bandwidth_resource_key,
                            ingress_bandwidth_resource_key,
                            mode: BandwidthMode::Strip,
                        }),
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
                    mutate::handler(
                        body,
                        Mode::Bandwidth(BandwidthProps {
                            egress_bandwidth_resource_key,
                            ingress_bandwidth_resource_key,
                            mode: BandwidthMode::Overwrite,
                        }),
                    )
                }
            }),
        )
        .route(
            "/namespace",
            post(move |body| mutate::handler(body, Mode::Scheduler(namespaces.clone()))),
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

// TODO: Add unit tests
