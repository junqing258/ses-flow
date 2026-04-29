use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, Utc};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::{RwLock, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::{
    AppConfig, DEFAULT_CONNECT_STATION_ID, DEFAULT_RUNNER_RESUME_SIGNAL, HEALTH_PLUGIN_ID, normalize_runner_base_url,
};
use crate::models::{
    CancelRequest, ConnectRequest, ExecuteRequest, ExecutionTask, HealthResponse, LoginRequest, PendingEvent,
    ResumeRequest, TaskErrorPayload, TaskSnapshot, TaskState, VerifyNotifyRequest, bearer_token,
};
use crate::views::{failure_resume_event, success_resume_event};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) inner: Arc<RwLock<BridgeState>>,
    event_seq: Arc<AtomicU64>,
    client: Client,
    db_pool: Option<PgPool>,
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

    pub(crate) async fn create_or_get_task(&self, request: ExecuteRequest) -> Result<ExecutionTask, String> {
        let target_station_id = resolve_station_id(&request)
            .ok_or_else(|| "missing workerId/stationId/targetWorkerId in config or input".to_string())?;
        let dedupe_key = task_lookup_key(&request.context.run_id, &request.node_id, &request.context.request_id);

        let maybe_existing = {
            let state = self.inner.read().await;
            state
                .task_keys
                .get(&dedupe_key)
                .and_then(|execution_id| state.tasks.get(execution_id))
                .cloned()
        };
        if let Some(task) = maybe_existing {
            return Ok(task);
        }

        let execution_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let runner_base_url = request
            .config
            .get("runnerBaseUrl")
            .and_then(Value::as_str)
            .map(|value| normalize_runner_base_url(value.to_string()))
            .or_else(|| self.config.runner_base_url.clone());
        let ses_base_url = resolve_ses_base_url(&request);
        let signal_type = request
            .config
            .get("waitSignalType")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_RUNNER_RESUME_SIGNAL)
            .to_string();

        let task = ExecutionTask {
            execution_id: execution_id.clone(),
            run_id: request.context.run_id.clone(),
            request_id: request.context.request_id.clone(),
            node_id: request.node_id.clone(),
            trace_id: request.context.trace_id.clone(),
            plugin_type: request.runner_type.clone(),
            plugin_id: request.plugin_id.clone(),
            target_station_id: target_station_id.clone(),
            payload: json!({
                "config": request.config,
                "input": request.context.input,
                "env": request.context.env,
                "sesBaseUrl": ses_base_url,
            }),
            task_id: resolve_task_id(&request).unwrap_or_else(|| execution_id.clone()),
            wait_signal_type: signal_type,
            state: TaskState::Pending,
            runner_base_url,
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::hours(12),
        };

        let event = self
            .queue_pending_event(
                &target_station_id,
                Some(execution_id.clone()),
                "task.dispatch",
                json!({
                    "ExecutionId": execution_id,
                    "PluginType": task.plugin_type,
                    "PluginId": task.plugin_id,
                    "TaskId": task.task_id,
                    "Payload": task.payload,
                    "ExpiresAt": task.expires_at.to_rfc3339(),
                    "RunId": task.run_id,
                    "NodeId": task.node_id
                }),
            )
            .await;

        {
            let mut state = self.inner.write().await;
            state.task_keys.insert(dedupe_key, task.execution_id.clone());
            state.tasks.insert(task.execution_id.clone(), task.clone());
        }

        info!(
            execution_id = %task.execution_id,
            station_id = %task.target_station_id,
            event_id = event.event_id,
            "queued WCS manual task"
        );

        Ok(task)
    }

    pub(crate) async fn queue_pending_event(
        &self,
        station_id: &str,
        execution_id: Option<String>,
        message_type: &str,
        payload: Value,
    ) -> PendingEvent {
        let event_id = self.event_seq.fetch_add(1, Ordering::SeqCst);
        let request_id = event_id.to_string();

        let (sender, event) = {
            let mut state = self.inner.write().await;
            let sender = state.worker_sender(station_id);
            let event = PendingEvent {
                event_id,
                request_id,
                station_id: station_id.to_string(),
                execution_id,
                message_type: message_type.to_string(),
                payload,
                acked_at: None,
                created_at: Utc::now(),
            };
            state
                .pending_events
                .entry(station_id.to_string())
                .or_default()
                .push(event.clone());
            (sender, event)
        };
        let _ = sender.send(event.clone());
        event
    }

    pub(crate) async fn simulate_agv_arrived(
        &self,
        station_id: &str,
        agv_id: &str,
        request_id: Option<Value>,
    ) -> AgvArrivalSimulation {
        let event_id = self.event_seq.fetch_add(1, Ordering::SeqCst);
        let runner_request_id = request_id.as_ref().and_then(value_to_string);
        let request_id_text = runner_request_id.clone().unwrap_or_else(|| event_id.to_string());
        let request_id_value = request_id.clone().unwrap_or_else(|| json!(event_id));
        let payload = json!({
            "MessageType": "AGV_ARRIVED",
            "messageType": "AGV_ARRIVED",
            "AgvId": agv_id,
            "StationId": station_id,
            "RequestId": request_id_value
        });

        let (sender, event) = {
            let mut state = self.inner.write().await;
            let sender = state.worker_sender(station_id);
            let event = PendingEvent {
                event_id,
                request_id: request_id_text.clone(),
                station_id: station_id.to_string(),
                execution_id: None,
                message_type: "AGV_ARRIVED".to_string(),
                payload,
                acked_at: None,
                created_at: Utc::now(),
            };
            state
                .pending_events
                .entry(station_id.to_string())
                .or_default()
                .push(event.clone());
            (sender, event)
        };
        let _ = sender.send(event.clone());
        info!(station_id = %station_id, agv_id = %agv_id, request_id = %request_id_text, "simulated AGV arrival");
        let resumed_run_ids = self
            .resume_agv_arrival_waits(station_id, agv_id, runner_request_id.as_deref())
            .await;
        AgvArrivalSimulation { event, resumed_run_ids }
    }

    pub(crate) async fn login(&self, request: &LoginRequest) -> Result<String, String> {
        if self.config.ses_auth_base_url.is_some() {
            return self.ses_station_login(request).await;
        }

        let token = Uuid::new_v4().to_string();
        let mut state = self.inner.write().await;
        state.tokens.insert(token.clone(), request.station_id.clone());
        Ok(token)
    }

    async fn ses_station_login(&self, request: &LoginRequest) -> Result<String, String> {
        let auth_base_url = self
            .config
            .ses_auth_base_url
            .as_ref()
            .ok_or_else(|| "SES auth base URL is not configured".to_string())?;
        let username = request
            .username
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "username is required".to_string())?;
        let password = request
            .password
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "password is required".to_string())?;
        let payload = json!({
            "stationId": request.station_id,
            "platformId": request.platform_id,
            "login": username,
            "password": password
        });
        let response = self
            .client
            .post(format!("{auth_base_url}/station-login"))
            .json(&payload)
            .send()
            .await
            .map_err(|error| format!("failed to call SES station login: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("failed to read SES station login response: {error}"))?;
        if !status.is_success() {
            return Err(ses_error_message(&body).unwrap_or_else(|| format!("SES station login failed: {status}")));
        }
        let payload: SesAuthPayload = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse SES station login response: {error}"))?;
        {
            let mut state = self.inner.write().await;
            state
                .tokens
                .insert(payload.access_token.clone(), request.station_id.clone());
        }
        Ok(payload.access_token)
    }

    pub(crate) async fn connect_context(
        &self,
        station_id: &str,
        since: Option<u64>,
    ) -> (broadcast::Receiver<PendingEvent>, Vec<PendingEvent>, Vec<TaskSnapshot>) {
        let mut state = self.inner.write().await;
        let receiver = state.worker_sender(station_id).subscribe();
        let backlog = state
            .pending_events
            .get(station_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.acked_at.is_none())
            .filter(|event| since.is_none_or(|cursor| event.event_id > cursor))
            .collect::<Vec<_>>();
        let snapshots = state
            .tasks
            .values()
            .filter(|task| task.target_station_id == station_id && !task.state.is_terminal())
            .map(TaskSnapshot::from)
            .collect::<Vec<_>>();
        (receiver, backlog, snapshots)
    }

    pub(crate) async fn verify_notify(&self, station_id: &str, request: VerifyNotifyRequest) -> Result<(), String> {
        let mut state = self.inner.write().await;
        let events = state.pending_events.entry(station_id.to_string()).or_default();
        let maybe_event = events.iter_mut().find(|event| {
            event.request_id == request.request_id
                && request
                    .execution_id
                    .as_ref()
                    .is_none_or(|execution_id| event.execution_id.as_deref() == Some(execution_id.as_str()))
        });
        let event = maybe_event.ok_or_else(|| "pending event not found".to_string())?;
        if event.acked_at.is_none() {
            event.acked_at = Some(Utc::now());
        }
        Ok(())
    }

    pub(crate) async fn current_task_for_worker(&self, station_id: &str) -> Option<ExecutionTask> {
        let state = self.inner.read().await;
        state
            .tasks
            .values()
            .filter(|task| task.target_station_id == station_id && !task.state.is_terminal())
            .max_by_key(|task| task.updated_at)
            .cloned()
    }

    pub(crate) async fn robot_departure_task_for_worker(
        &self,
        station_id: &str,
        task_id: &str,
    ) -> Option<ExecutionTask> {
        let state = self.inner.read().await;
        state
            .tasks
            .values()
            .filter(|task| {
                task.target_station_id == station_id
                    && !task.state.is_terminal()
                    && task.plugin_type == "plugin:robot_departure"
                    && task_matches_task_id(task, task_id)
            })
            .max_by_key(|task| task.updated_at)
            .cloned()
    }

    pub(crate) async fn complete_task_with_success(
        &self,
        station_id: &str,
        request_id: String,
        output: Value,
        state_patch: Value,
        agv_depart_payload: Option<Value>,
    ) -> Result<(), String> {
        let task = self
            .current_task_for_worker(station_id)
            .await
            .ok_or_else(|| "no active task for worker".to_string())?;
        self.transition_task_success(&task.execution_id, output, state_patch)
            .await?;
        if let Some(payload) = agv_depart_payload {
            self.queue_pending_event(station_id, Some(task.execution_id), "AGV_DEPART", payload)
                .await;
        }
        info!(station_id = %station_id, request_id = %request_id, "worker completed WCS task");
        Ok(())
    }

    pub(crate) async fn complete_task_by_execution_id_with_success(
        &self,
        execution_id: &str,
        station_id: &str,
        request_id: String,
        output: Value,
        state_patch: Value,
        agv_depart_payload: Option<Value>,
    ) -> Result<(), String> {
        self.transition_task_success(execution_id, output, state_patch).await?;
        if let Some(payload) = agv_depart_payload {
            self.queue_pending_event(station_id, Some(execution_id.to_string()), "AGV_DEPART", payload)
                .await;
        }
        info!(station_id = %station_id, request_id = %request_id, execution_id = %execution_id, "worker completed selected WCS task");
        Ok(())
    }

    pub(crate) async fn fail_task(&self, execution_id: &str, error: TaskErrorPayload) -> Result<(), String> {
        let task = {
            let mut state = self.inner.write().await;
            let task = state
                .tasks
                .get_mut(execution_id)
                .ok_or_else(|| "execution task not found".to_string())?;
            task.state = TaskState::Failed;
            task.updated_at = Utc::now();
            task.clone()
        };

        self.resume_runner(&task, failure_resume_event(&task, &error)).await
    }

    pub(crate) async fn cancel_task(&self, request: CancelRequest) -> Result<(), String> {
        let task = {
            let state = self.inner.read().await;
            let execution_id = state
                .task_keys
                .get(&task_lookup_key(&request.run_id, &request.node_id, &request.request_id))
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?;
            state
                .tasks
                .get(&execution_id)
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?
        };
        self.queue_pending_event(
            &task.target_station_id,
            Some(task.execution_id.clone()),
            "task.cancel",
            json!({
                "ExecutionId": task.execution_id,
                "Reason": request.reason.unwrap_or_else(|| "workflow_terminated".to_string())
            }),
        )
        .await;
        Ok(())
    }

    pub(crate) async fn resume_external(&self, request: ResumeRequest) -> Result<(), String> {
        let task = {
            let state = self.inner.read().await;
            let execution_id = state
                .task_keys
                .get(&task_lookup_key(&request.run_id, &request.node_id, &request.request_id))
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?;
            state
                .tasks
                .get(&execution_id)
                .cloned()
                .ok_or_else(|| "execution task not found".to_string())?
        };
        self.queue_pending_event(
            &task.target_station_id,
            Some(task.execution_id.clone()),
            "task.resume",
            json!({
                "ExecutionId": task.execution_id,
                "Signal": request.signal.unwrap_or(Value::Null)
            }),
        )
        .await;
        Ok(())
    }

    pub(crate) async fn authenticated_station_id(&self, headers: &axum::http::HeaderMap) -> Result<String, String> {
        if self.config.ses_auth_base_url.is_some() {
            let token = bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
            return self.ses_authenticated_station_id(&token).await;
        }

        let state = self.inner.read().await;
        if let Some(token) = bearer_token(headers) {
            if let Some(station_id) = state.tokens.get(&token).cloned() {
                return Ok(station_id);
            }
        }

        let mut connected_workers = state
            .worker_streams
            .keys()
            .filter(|station_id| station_id.as_str() != DEFAULT_CONNECT_STATION_ID);
        let fallback_station_id = connected_workers.next().cloned();
        if fallback_station_id.is_some() && connected_workers.next().is_none() {
            let station_id = fallback_station_id.expect("fallback worker id should exist");
            warn!(station_id = %station_id, "模拟登录验证");
            return Ok(station_id);
        }

        if bearer_token(headers).is_some() {
            Err("invalid bearer token".to_string())
        } else {
            Err("missing bearer token".to_string())
        }
    }

    async fn ses_authenticated_station_id(&self, token: &str) -> Result<String, String> {
        let auth_base_url = self
            .config
            .ses_auth_base_url
            .as_ref()
            .ok_or_else(|| "SES auth base URL is not configured".to_string())?;
        let response = self
            .client
            .post(format!("{auth_base_url}/station-authorize"))
            .bearer_auth(token)
            .json(&json!({ "requiredPermission": "workstation.operate" }))
            .send()
            .await
            .map_err(|error| format!("failed to call SES station authorization: {error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("failed to read SES station authorization response: {error}"))?;
        if !status.is_success() {
            return Err(
                ses_error_message(&body).unwrap_or_else(|| format!("SES station authorization failed: {status}"))
            );
        }
        let payload: SesStationAuthorizeResponse = serde_json::from_str(&body)
            .map_err(|error| format!("failed to parse SES station authorization response: {error}"))?;
        {
            let mut state = self.inner.write().await;
            state.tokens.insert(token.to_string(), payload.station_id.clone());
        }
        Ok(payload.station_id)
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

    async fn transition_task_success(
        &self,
        execution_id: &str,
        output: Value,
        state_patch: Value,
    ) -> Result<(), String> {
        let task = {
            let mut state = self.inner.write().await;
            let task = state
                .tasks
                .get_mut(execution_id)
                .ok_or_else(|| "execution task not found".to_string())?;
            task.state = TaskState::Succeeded;
            task.updated_at = Utc::now();
            task.clone()
        };

        self.resume_runner(&task, success_resume_event(&task, output, state_patch))
            .await
    }

    async fn resume_runner(&self, task: &ExecutionTask, event: Value) -> Result<(), String> {
        let Some(base_url) = task.runner_base_url.as_ref() else {
            warn!(execution_id = %task.execution_id, "runner resume skipped because RUNNER_BASE_URL is not configured");
            return Ok(());
        };
        let response = self
            .client
            .post(format!(
                "{}/runs/{}/resume",
                base_url.trim_end_matches('/'),
                task.run_id
            ))
            .json(&json!({ "event": event }))
            .send()
            .await
            .map_err(|error| format!("failed to call runner resume endpoint: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "runner resume endpoint returned {} for execution {}",
                response.status(),
                task.execution_id
            ));
        }
        Ok(())
    }

    async fn resume_agv_arrival_waits(&self, station_id: &str, agv_id: &str, request_id: Option<&str>) -> Vec<String> {
        let Some(base_url) = self.config.runner_base_url.as_ref() else {
            warn!(station_id = %station_id, "AGV arrival resume skipped because RUNNER_BASE_URL is not configured");
            return Vec::new();
        };
        let run_ids = self.search_wait_run_ids(station_id, "agv.arrived", request_id).await;

        let mut resumed_run_ids = Vec::new();
        for run_id in run_ids {
            if self
                .resume_agv_arrival_run(base_url, &run_id, station_id, agv_id, request_id)
                .await
            {
                resumed_run_ids.push(run_id.to_string());
            }
        }
        resumed_run_ids
    }

    pub(crate) async fn resume_scan_barcode_waits(
        &self,
        station_id: &str,
        barcode: &str,
        sku: &str,
        request_id: Option<&str>,
    ) -> Vec<String> {
        let Some(base_url) = self.config.runner_base_url.as_ref() else {
            warn!(station_id = %station_id, barcode = %barcode, "scan barcode resume skipped because RUNNER_BASE_URL is not configured");
            return Vec::new();
        };
        let run_ids = self.search_scan_barcode_wait_run_ids(station_id, barcode).await;

        let mut resumed_run_ids = Vec::new();
        for run_id in run_ids {
            if self
                .resume_scan_barcode_run(base_url, &run_id, station_id, barcode, sku, request_id)
                .await
            {
                resumed_run_ids.push(run_id.to_string());
            }
        }
        resumed_run_ids
    }

    pub(crate) async fn resume_robot_departure_waits(
        &self,
        station_id: &str,
        task_id: &str,
        agv_id: &str,
        completed: i64,
        request_id: &str,
    ) -> Vec<String> {
        let Some(base_url) = self.config.runner_base_url.as_ref() else {
            warn!(station_id = %station_id, task_id = %task_id, "robot departure resume skipped because RUNNER_BASE_URL is not configured");
            return Vec::new();
        };
        let run_ids = self.search_robot_departure_wait_run_ids(station_id, task_id).await;

        let mut resumed_run_ids = Vec::new();
        for run_id in run_ids {
            if self
                .resume_robot_departure_run(base_url, &run_id, task_id, agv_id, completed, request_id)
                .await
            {
                resumed_run_ids.push(run_id.to_string());
            }
        }
        resumed_run_ids
    }

    pub(crate) async fn record_pending_robot_departure(
        &self,
        station_id: &str,
        task_id: &str,
        agv_id: &str,
        completed: i64,
        request_id: &str,
    ) {
        let mut state = self.inner.write().await;
        state.pending_robot_departures.insert(
            robot_departure_key(station_id, task_id),
            PendingRobotDeparture {
                station_id: station_id.to_string(),
                task_id: task_id.to_string(),
                agv_id: agv_id.to_string(),
                completed,
                request_id: request_id.to_string(),
            },
        );
    }

    pub(crate) async fn take_pending_robot_departure(
        &self,
        station_id: &str,
        task_id: &str,
    ) -> Option<PendingRobotDeparture> {
        let mut state = self.inner.write().await;
        state
            .pending_robot_departures
            .remove(&robot_departure_key(station_id, task_id))
    }

    async fn search_wait_run_ids(&self, station_id: &str, event: &str, request_id: Option<&str>) -> Vec<String> {
        let Some(db_pool) = self.db_pool.as_ref() else {
            warn!(station_id = %station_id, event = %event, "waiting run search skipped because DATABASE_URL is not configured");
            return Vec::new();
        };

        let rows = match sqlx::query(
            r#"
            SELECT run_id
            FROM workflow_runs
            WHERE status IN ($1, $2)
              AND last_signal->>'type' = $3
              AND last_signal->'payload'->>'stationId' = $4
              AND (
                $5::text IS NULL
                OR last_signal->'payload'->>'requestId' = $5
                OR last_signal->'payload'->>'correlationKey' = $5
              )
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind("\"waiting\"")
        .bind("waiting")
        .bind(event)
        .bind(station_id)
        .bind(request_id)
        .fetch_all(db_pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                warn!(station_id = %station_id, event = %event, error = %error, "failed to search waiting runs from database");
                return Vec::new();
            }
        };

        rows.into_iter()
            .filter_map(|row| row.try_get::<String, _>("run_id").ok())
            .collect()
    }

    async fn search_scan_barcode_wait_run_ids(&self, station_id: &str, barcode: &str) -> Vec<String> {
        let Some(db_pool) = self.db_pool.as_ref() else {
            warn!(station_id = %station_id, barcode = %barcode, "scan barcode run search skipped because DATABASE_URL is not configured");
            return Vec::new();
        };

        let event = "station.operation.scanBarcode";
        let unique_key = scan_barcode_unique_key(station_id, barcode);
        let rows = match sqlx::query(
            r#"
            SELECT run_id
            FROM workflow_runs
            WHERE status IN ($1, $2)
              AND last_signal->>'type' = $3
              AND unique_key = $4
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind("\"waiting\"")
        .bind("waiting")
        .bind(event)
        .bind(&unique_key)
        .fetch_all(db_pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                warn!(
                    station_id = %station_id,
                    barcode = %barcode,
                    unique_key = %unique_key,
                    error = %error,
                    "failed to search scan barcode waiting runs from database"
                );
                return Vec::new();
            }
        };

        rows.into_iter()
            .filter_map(|row| row.try_get::<String, _>("run_id").ok())
            .collect()
    }

    async fn search_robot_departure_wait_run_ids(&self, station_id: &str, task_id: &str) -> Vec<String> {
        let Some(db_pool) = self.db_pool.as_ref() else {
            warn!(station_id = %station_id, task_id = %task_id, "robot departure run search skipped because DATABASE_URL is not configured");
            return Vec::new();
        };

        let rows = match sqlx::query(
            r#"
            SELECT run_id
            FROM workflow_runs
            WHERE status IN ($1, $2)
              AND current_node_id = $3
              AND last_signal->>'type' = $4
              AND state->'workstation'->'lastRobotDeparture'->>'taskId' = $5
              AND EXISTS (
                SELECT 1
                FROM jsonb_each(state->'workstation'->'executions') AS execution(id, payload)
                WHERE execution.payload->>'workerId' = $6
              )
            ORDER BY updated_at DESC
            LIMIT 100
            "#,
        )
        .bind("\"waiting\"")
        .bind("waiting")
        .bind("robot_departure")
        .bind(DEFAULT_RUNNER_RESUME_SIGNAL)
        .bind(task_id)
        .bind(station_id)
        .fetch_all(db_pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                warn!(station_id = %station_id, task_id = %task_id, error = %error, "failed to search robot departure waiting runs from database");
                return Vec::new();
            }
        };

        rows.into_iter()
            .filter_map(|row| row.try_get::<String, _>("run_id").ok())
            .collect()
    }

    async fn resume_agv_arrival_run(
        &self,
        base_url: &str,
        run_id: &str,
        station_id: &str,
        agv_id: &str,
        request_id: Option<&str>,
    ) -> bool {
        let event = agv_arrival_resume_event(station_id, agv_id, request_id);
        let response = self
            .client
            .post(format!("{}/runs/{}/resume", base_url.trim_end_matches('/'), run_id))
            .json(&json!({ "event": event }))
            .send()
            .await;

        match response {
            Ok(response) if response.status().is_success() => {
                info!(run_id = %run_id, station_id = %station_id, agv_id = %agv_id, "resumed AGV arrival wait");
                true
            }
            Ok(response) => {
                warn!(
                    run_id = %run_id,
                    station_id = %station_id,
                    status = %response.status(),
                    "runner resume returned non-success status for AGV arrival"
                );
                false
            }
            Err(error) => {
                warn!(run_id = %run_id, station_id = %station_id, error = %error, "failed to resume AGV arrival wait");
                false
            }
        }
    }

    async fn resume_scan_barcode_run(
        &self,
        base_url: &str,
        run_id: &str,
        station_id: &str,
        barcode: &str,
        sku: &str,
        request_id: Option<&str>,
    ) -> bool {
        let event = scan_barcode_resume_event(station_id, barcode, sku, request_id);
        let response = self
            .client
            .post(format!("{}/runs/{}/resume", base_url.trim_end_matches('/'), run_id))
            .json(&json!({ "event": event }))
            .send()
            .await;

        match response {
            Ok(response) if response.status().is_success() => {
                info!(run_id = %run_id, station_id = %station_id, barcode = %barcode, "resumed scan barcode wait");
                true
            }
            Ok(response) => {
                warn!(
                    run_id = %run_id,
                    station_id = %station_id,
                    barcode = %barcode,
                    status = %response.status(),
                    "runner resume returned non-success status for scan barcode"
                );
                false
            }
            Err(error) => {
                warn!(run_id = %run_id, station_id = %station_id, barcode = %barcode, error = %error, "failed to resume scan barcode wait");
                false
            }
        }
    }

    async fn resume_robot_departure_run(
        &self,
        base_url: &str,
        run_id: &str,
        task_id: &str,
        agv_id: &str,
        completed: i64,
        request_id: &str,
    ) -> bool {
        let response = self
            .client
            .post(format!("{}/runs/{}/resume", base_url.trim_end_matches('/'), run_id))
            .json(&json!({
                "event": {
                    "type": DEFAULT_RUNNER_RESUME_SIGNAL,
                    "payload": {
                        "status": "success",
                        "output": {
                            "taskId": task_id,
                            "agvId": agv_id,
                            "completed": completed,
                            "requestId": request_id
                        },
                        "statePatch": {
                            "workstation": {
                                "lastRobotDeparture": {
                                    "taskId": task_id,
                                    "agvId": agv_id,
                                    "completed": completed,
                                    "requestId": request_id
                                }
                            }
                        }
                    }
                }
            }))
            .send()
            .await;

        match response {
            Ok(response) if response.status().is_success() => {
                info!(run_id = %run_id, task_id = %task_id, agv_id = %agv_id, "resumed robot departure wait");
                true
            }
            Ok(response) => {
                warn!(
                    run_id = %run_id,
                    task_id = %task_id,
                    agv_id = %agv_id,
                    status = %response.status(),
                    "runner resume returned non-success status for robot departure"
                );
                false
            }
            Err(error) => {
                warn!(run_id = %run_id, task_id = %task_id, agv_id = %agv_id, error = %error, "failed to resume robot departure wait");
                false
            }
        }
    }
}

pub(crate) struct AgvArrivalSimulation {
    pub(crate) event: PendingEvent,
    pub(crate) resumed_run_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingRobotDeparture {
    pub(crate) station_id: String,
    pub(crate) task_id: String,
    pub(crate) agv_id: String,
    pub(crate) completed: i64,
    pub(crate) request_id: String,
}

fn agv_arrival_resume_event(station_id: &str, agv_id: &str, request_id: Option<&str>) -> Value {
    let mut event = json!({
        "event": "agv.arrived",
        "stationId": station_id,
        "agvId": agv_id,
        "payload": {
            "stationId": station_id,
            "agvId": agv_id
        }
    });
    attach_request_id(&mut event, request_id);
    event
}

fn scan_barcode_resume_event(station_id: &str, barcode: &str, sku: &str, request_id: Option<&str>) -> Value {
    let mut event = json!({
        "event": "station.operation.scanBarcode",
        "stationId": station_id,
        "barcode": barcode,
        "itemId": barcode,
        "sku": sku,
        "payload": {
            "stationId": station_id,
            "barcode": barcode,
            "itemId": barcode,
            "sku": sku
        }
    });
    attach_request_id(&mut event, request_id);
    event
}

fn scan_barcode_unique_key(station_id: &str, barcode: &str) -> String {
    format!("workstation:{station_id}:{barcode}")
}

fn attach_request_id(event: &mut Value, request_id: Option<&str>) {
    let Some(request_id) = request_id else {
        return;
    };
    if let Some(event_object) = event.as_object_mut() {
        event_object.insert("requestId".to_string(), json!(request_id));
    }
    if let Some(payload_object) = event.get_mut("payload").and_then(Value::as_object_mut) {
        payload_object.insert("requestId".to_string(), json!(request_id));
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SesAuthPayload {
    access_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SesStationAuthorizeResponse {
    station_id: String,
}

fn ses_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|payload| payload.get("error").and_then(Value::as_str).map(str::to_string))
        .filter(|message| !message.trim().is_empty())
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
    fn worker_sender(&mut self, station_id: &str) -> broadcast::Sender<PendingEvent> {
        self.worker_streams
            .entry(station_id.to_string())
            .or_insert_with(|| broadcast::channel(128).0)
            .clone()
    }
}

pub(crate) fn station_id_from_connect(request: &ConnectRequest) -> String {
    request
        .station_id
        .clone()
        .or_else(|| request.station_ids.first().cloned())
        .or_else(|| request.client_id.clone())
        .unwrap_or_else(|| DEFAULT_CONNECT_STATION_ID.to_string())
}

fn resolve_station_id(request: &ExecuteRequest) -> Option<String> {
    value_string(&request.config, &["workerId", "stationId", "targetWorkerId"])
        .or_else(|| value_string(&request.context.input, &["workerId", "stationId", "targetWorkerId"]))
        .or_else(|| value_string(&request.context.state, &["workerId", "stationId", "targetWorkerId"]))
        .or_else(|| workstation_execution_station_id(&request.context.state))
}

fn resolve_ses_base_url(request: &ExecuteRequest) -> Option<String> {
    value_string(&request.context.env, &["sesBaseUrl"])
}

fn resolve_task_id(request: &ExecuteRequest) -> Option<String> {
    value_string(&request.config, &["taskId"]).or_else(|| value_string(&request.context.input, &["taskId"]))
}

fn task_matches_task_id(task: &ExecutionTask, task_id: &str) -> bool {
    task.task_id == task_id
        || value_string(&task.payload, &["taskId"]).as_deref() == Some(task_id)
        || task
            .payload
            .get("input")
            .and_then(|input| value_string(input, &["taskId"]))
            .as_deref()
            == Some(task_id)
        || task
            .payload
            .get("config")
            .and_then(|config| value_string(config, &["taskId"]))
            .as_deref()
            == Some(task_id)
}

fn robot_departure_key(station_id: &str, task_id: &str) -> String {
    format!("{station_id}:{task_id}")
}

fn task_lookup_key(run_id: &str, node_id: &str, request_id: &str) -> String {
    format!("{run_id}:{node_id}:{request_id}")
}

fn value_string(value: &Value, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|key| {
        value.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn workstation_execution_station_id(state: &Value) -> Option<String> {
    let mut station_ids = state
        .get("workstation")
        .and_then(|workstation| workstation.get("executions"))
        .and_then(Value::as_object)?
        .values()
        .filter_map(|execution| execution.get("workerId").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    station_ids.sort();
    station_ids.dedup();
    match station_ids.as_slice() {
        [station_id] => Some(station_id.clone()),
        _ => None,
    }
}
