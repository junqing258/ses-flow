use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use runner::api::{ApiState, build_router};
use runner::server::WorkflowServer;
use runner::store::{PostgresCatalogStore, PostgresEditSessionStore, PostgresRunStore};
use runner::utils::telemetry::init_tracing;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Load .env file if it exists
    dotenv::dotenv().ok();

    init_tracing();

    if let Err(error) = run().await {
        error!(error = %error, "runner failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let host = parse_arg("--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_arg("--port").unwrap_or_else(|| "3002".to_string());
    let database_url = parse_arg("--database-url")
        .or_else(|| env::var("DATABASE_URL").ok())
        .unwrap_or_else(|| "postgresql://runner:runner@localhost/flow-runner".to_string());

    let address: SocketAddr = format!("{host}:{port}").parse()?;

    info!(database_url = %database_url, "initializing PostgreSQL stores");
    let run_store = Arc::new(PostgresRunStore::new(&database_url).await?);
    let catalog_store = Arc::new(PostgresCatalogStore::new(run_store.get_pool()).await?);
    let edit_session_store = Arc::new(PostgresEditSessionStore::new(run_store.get_pool()).await?);

    let router = build_router(ApiState {
        server: Arc::new(WorkflowServer::with_store_catalog_and_sessions(
            run_store,
            catalog_store,
            edit_session_store,
        )),
    });
    let listener = tokio::net::TcpListener::bind(address).await?;
    info!(address = %address, "runner api listening");
    axum::serve(listener, router).await?;
    Ok(())
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}
