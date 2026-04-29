use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use backend::modules::auth::{AuthService, PostgresAuthStore};
use backend::modules::node_registry::register_http_plugin_base_urls;
use backend::modules::system::system_store::PostgresSystemSettingsStore;
use backend::modules::{ApiState, ai_gateway, build_router};
use runner::app::WorkflowApp;
use runner::config::RunnerConfig;
use runner::store::{PostgresCatalogStore, PostgresEditSessionStore, PostgresRunStore};
use ses_flow_telemetry::init_tracing;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let _log_guard = init_tracing();

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
    let config_path = parse_arg("--config")
        .map(PathBuf::from)
        .or_else(|| env::var("RUNNER_CONFIG_PATH").ok().map(PathBuf::from));
    let config = RunnerConfig::load_optional(config_path.as_ref())?;

    let address: SocketAddr = format!("{host}:{port}").parse()?;

    info!(database_url = %database_url, "initializing PostgreSQL stores");
    let run_store = Arc::new(PostgresRunStore::new(&database_url).await?);
    let pool = run_store.get_pool();
    let catalog_store = Arc::new(PostgresCatalogStore::new(pool.clone()).await?);
    let edit_session_store = Arc::new(PostgresEditSessionStore::new(pool.clone()).await?);
    let system_settings = Arc::new(
        PostgresSystemSettingsStore::new(pool)
            .await
            .map_err(std::io::Error::other)?,
    );
    let auth = AuthService::new(Arc::new(
        PostgresAuthStore::new(run_store.get_pool())
            .await
            .map_err(std::io::Error::other)?,
    ));
    auth.bootstrap_from_env().await.map_err(std::io::Error::other)?;

    let state = ApiState {
        app: Arc::new(WorkflowApp::with_store_catalog_sessions_and_concurrency(
            run_store,
            catalog_store,
            edit_session_store,
            config.concurrency,
        )),
        ai_gateway_base_url: ai_gateway::resolve_ai_gateway_base_url(),
        ai_gateway_client: reqwest::Client::new(),
        system_settings,
        auth,
        auth_required: true,
    };
    let auto_register_plugin_base_urls = state
        .system_settings
        .load_plugin_auto_register_base_urls()
        .await
        .map_err(std::io::Error::other)?;
    if !auto_register_plugin_base_urls.is_empty() {
        match register_http_plugin_base_urls(&state, &auto_register_plugin_base_urls).await {
            Ok(descriptors) => {
                info!(
                    count = descriptors.len(),
                    plugin_ids = ?descriptors.iter().map(|descriptor| descriptor.id.clone()).collect::<Vec<_>>(),
                    "auto-registered http plugins"
                );
            }
            Err(error) => {
                warn!(
                    error = ?error,
                    base_urls = ?auto_register_plugin_base_urls,
                    "failed to auto-register http plugins; continuing backend startup"
                );
            }
        }
    }

    let router = build_router(state);
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
