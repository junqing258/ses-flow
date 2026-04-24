use std::env;
use std::net::SocketAddr;

use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use wcs_plugin::{AppConfig, build_app_with_config};

#[tokio::main]
async fn main() {
    init_tracing();

    if let Err(error) = run().await {
        eprintln!("wcs-plugin failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let host = parse_arg("--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_arg("--port").unwrap_or_else(|| "9102".to_string());
    let address: SocketAddr = format!("{host}:{port}").parse()?;

    let listener = tokio::net::TcpListener::bind(address).await?;
    let config = AppConfig::from_env();
    info!(address = %address, runner_base_url = ?config.runner_base_url, "wcs plugin listening");
    axum::serve(listener, build_app_with_config(config)).await?;
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    if tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init()
        .is_err()
    {
        warn!("tracing subscriber already initialized");
    }
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}
