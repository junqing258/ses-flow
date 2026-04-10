use std::env;
use std::sync::Once;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

static TRACING_INIT: Once = Once::new();

pub fn init_tracing() {
    TRACING_INIT.call_once(|| {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter()));

        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_thread_names(true)
                    .compact(),
            )
            .init();
    });
}

fn default_filter() -> String {
    env::var("RUNNER_LOG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "runner=info".to_string())
}
