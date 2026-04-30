//! 服务共享状态。
//! 定义 AppState、内存态 BridgeState、数据库连接、HTTP 客户端和健康检查统计。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use reqwest::Client;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::{RwLock, broadcast};
use tracing::warn;

use crate::config::{AppConfig, HEALTH_PLUGIN_ID};
use crate::models::{ExecutionTask, HealthResponse, PendingEvent};

use super::PendingRobotDeparture;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) inner: Arc<RwLock<BridgeState>>,
    pub(super) event_seq: Arc<AtomicU64>,
    pub(super) client: Client,
    pub(super) db_pool: Option<PgPool>,
}

impl AppState {
    pub(crate) fn new(config: AppConfig) -> Self {
        let db_pool = config.database_url.as_ref().and_then(|database_url| {
            match PgPoolOptions::new().max_connections(2).connect_lazy(database_url) {
                Ok(pool) => Some(pool),
                Err(error) => {
                    warn!(error = %error, "failed to create lazy database pool for workstation plugin");
                    None
                }
            }
        });

        Self {
            config,
            inner: Arc::new(RwLock::new(BridgeState::default())),
            event_seq: Arc::new(AtomicU64::new(1)),
            client: Client::new(),
            db_pool,
        }
    }

    pub(crate) fn heartbeat_interval_secs(&self) -> u64 {
        self.config.heartbeat_interval_secs
    }

    pub(crate) async fn health(&self) -> HealthResponse {
        let state = self.inner.read().await;
        let pending_events = state
            .pending_events
            .values()
            .flat_map(|events| events.iter())
            .filter(|event| event.acked_at.is_none())
            .count();
        HealthResponse {
            status: "ok".to_string(),
            plugin_id: HEALTH_PLUGIN_ID.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            online_workers: state.worker_streams.len(),
            active_tasks: state.tasks.values().filter(|task| !task.state.is_terminal()).count(),
            pending_events,
        }
    }
}

#[derive(Default)]
pub(crate) struct BridgeState {
    pub(crate) tasks: HashMap<String, ExecutionTask>,
    pub(crate) task_keys: HashMap<String, String>,
    pub(crate) tokens: HashMap<String, String>,
    pub(crate) worker_streams: HashMap<String, broadcast::Sender<PendingEvent>>,
    pub(crate) pending_events: HashMap<String, Vec<PendingEvent>>,
    pub(crate) pending_robot_departures: HashMap<String, PendingRobotDeparture>,
}

impl BridgeState {
    pub(super) fn worker_sender(&mut self, station_id: &str) -> broadcast::Sender<PendingEvent> {
        self.worker_streams
            .entry(station_id.to_string())
            .or_insert_with(|| broadcast::channel(128).0)
            .clone()
    }
}
