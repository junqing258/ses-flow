use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use backend::modules::{ApiState, ai_gateway, build_router};
use runner::app::WorkflowApp;
use runner::store::{PostgresCatalogStore, PostgresEditSessionStore, PostgresRunStore};
use runner::utils::telemetry::init_tracing;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    init_tracing();

    if let Err(error) = run().await {
        error!(error = %error, "backend failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let host = parse_arg("--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_arg("--port").unwrap_or_else(|| "6302".to_string());
    let database_url = parse_arg("--database-url")
        .or_else(|| env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgresql://runner:runner@localhost/flow-runner".to_string());

    let address: SocketAddr = format!("{host}:{port}").parse()?;

    info!(database_url = %database_url, "initializing PostgreSQL stores");
    let run_store = Arc::new(PostgresRunStore::new(&database_url).await?);
    let catalog_store = Arc::new(PostgresCatalogStore::new(run_store.get_pool()).await?);
    let edit_session_store = Arc::new(PostgresEditSessionStore::new(run_store.get_pool()).await?);

    let router = build_router(ApiState {
        app: Arc::new(WorkflowApp::with_store_catalog_and_sessions(
            run_store,
            catalog_store,
            edit_session_store,
        )),
        ai_gateway_base_url: ai_gateway::resolve_ai_gateway_base_url(),
        ai_gateway_client: reqwest::Client::new(),
    });
    let listener = tokio::net::TcpListener::bind(address).await?;
    info!(address = %address, "backend listening");
    axum::serve(listener, router).await?;
    Ok(())
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}
