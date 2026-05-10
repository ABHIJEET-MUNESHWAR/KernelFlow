//! KernelFlow node binary. Boots logging + metrics, opens the storage shard,
//! starts the GraphQL API, and (optionally) joins the libp2p mesh.

use std::sync::Arc;

use clap::Parser;
use kernelflow_api::{AppState, build_router};
use kernelflow_core::KernelEvent;
use tokio::sync::broadcast;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "kernelflow", version)]
struct Cli {
    /// HTTP listen address for GraphQL.
    #[arg(long, env = "KF_HTTP_ADDR", default_value = "0.0.0.0:8080")]
    http_addr: String,

    /// libp2p listen multiaddr (set empty to disable).
    #[arg(long, env = "KF_P2P_ADDR", default_value = "/ip4/0.0.0.0/tcp/0")]
    p2p_addr: String,

    /// RocksDB data directory.
    #[arg(long, env = "KF_DATA_DIR", default_value = "./data")]
    data_dir: String,

    /// Prometheus metrics port.
    #[arg(long, env = "KF_METRICS_PORT", default_value_t = 9090)]
    metrics_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let cli = Cli::parse();

    // Metrics
    let _ = metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], cli.metrics_port))
        .install();

    // Event bus (event-driven μsvc backbone).
    let (events_tx, _) = broadcast::channel::<KernelEvent>(2048);
    let state = Arc::new(AppState { events: events_tx.clone() });

    // GraphQL server
    let app = build_router(state.clone());
    let listener = tokio::net::TcpListener::bind(&cli.http_addr).await?;
    tracing::info!(addr = %cli.http_addr, "kernelflow node listening");

    axum::serve(listener, app).await?;
    Ok(())
}

