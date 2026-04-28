use std::env;
use std::net::SocketAddr;

use runner::utils::telemetry::init_tracing_with_service_name;
use tracing::info;
use workstation_plugin::{AppConfig, build_app_with_config};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let _telemetry_guard = init_tracing_with_service_name("ses-flow-workstation-plugin");

    if let Err(error) = run().await {
        eprintln!("workstation-plugin failed: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let host = parse_arg("--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_arg("--port").unwrap_or_else(|| "9102".to_string());
    let address: SocketAddr = format!("{host}:{port}").parse()?;

    let listener = tokio::net::TcpListener::bind(address).await?;
    let config = AppConfig::from_env();
    info!(
        address = %address,
        runner_base_url = ?config.runner_base_url,
        database_configured = config.database_url.is_some(),
        "workstation plugin listening"
    );
    axum::serve(listener, build_app_with_config(config)).await?;
    Ok(())
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}
