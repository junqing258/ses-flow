use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use runner::api::{ApiState, build_router};
use runner::server::WorkflowServer;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let host = parse_arg("--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parse_arg("--port").unwrap_or_else(|| "3002".to_string());

    let address: SocketAddr = format!("{host}:{port}").parse()?;
    let router = build_router(ApiState {
        server: Arc::new(WorkflowServer::new()),
    });
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("runner api listening on http://{address}");
    axum::serve(listener, router).await?;
    Ok(())
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}
