use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::{Value, json};
use tokio::sync::{RwLock, broadcast};
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::{
    AppConfig, DEFAULT_CONNECT_WORKER_ID, DEFAULT_RUNNER_RESUME_SIGNAL, HEALTH_PLUGIN_ID, normalize_runner_base_url,
};
use crate::models::{
    CancelRequest, ConnectRequest, ExecuteRequest, ExecutionTask, HealthResponse, PendingEvent, ResumeRequest,
    TaskErrorPayload, TaskSnapshot, TaskState, VerifyNotifyRequest, bearer_token,
};
use crate::views::{failure_resume_event, success_resume_event};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: AppConfig,
    pub(crate) inner: Arc<RwLock<BridgeState>>,
    event_seq: Arc<AtomicU64>,
    client: Client,
}

impl AppState {
    pub(crate) fn new(config: AppConfig) -> Self {
        Self {
            config,
            inner: Arc::new(RwLock::new(BridgeState::default())),
            event_seq: Arc::new(AtomicU64::new(1)),
            client: Client::new(),
        }
    }

    pub(crate) fn heartbeat_interval_secs(&self) -> u64 {
        self.config.heartbeat_interval_secs
    }

    pub(crate) async fn create_or_get_task(&self, request: ExecuteRequest) -> Result<ExecutionTask, String> {
        let target_worker_id = resolve_worker_id(&request)
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
            target_worker_id: target_worker_id.clone(),
            payload: json!({
                "config": request.config,
                "input": request.context.input,
                "env": request.context.env,
            }),
            task_id: request
                .config
                .get("taskId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| execution_id.clone()),
            wait_signal_type: signal_type,
            state: TaskState::Pending,
            runner_base_url,
            created_at: now,
            updated_at: now,
            expires_at: now + Duration::hours(12),
        };

        let event = self
            .queue_pending_event(
                &target_worker_id,
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
            worker_id = %task.target_worker_id,
            event_id = event.event_id,
            "queued WCS manual task"
        );

        Ok(task)
    }

    pub(crate) async fn queue_pending_event(
        &self,
        worker_id: &str,
        execution_id: Option<String>,
        message_type: &str,
        payload: Value,
    ) -> PendingEvent {
        let event_id = self.event_seq.fetch_add(1, Ordering::SeqCst);
        let request_id = event_id.to_string();

        let (sender, event) = {
            let mut state = self.inner.write().await;
            let sender = state.worker_sender(worker_id);
            let event = PendingEvent {
                event_id,
                request_id,
                worker_id: worker_id.to_string(),
                execution_id,
                message_type: message_type.to_string(),
                payload,
                acked_at: None,
                created_at: Utc::now(),
            };
            state
                .pending_events
                .entry(worker_id.to_string())
                .or_default()
                .push(event.clone());
            (sender, event)
        };
        let _ = sender.send(event.clone());
        event
    }

    pub(crate) async fn login(&self, worker_id: &str) -> String {
        let token = Uuid::new_v4().to_string();
        let mut state = self.inner.write().await;
        state.tokens.insert(token.clone(), worker_id.to_string());
        token
    }

    pub(crate) async fn connect_context(
        &self,
        worker_id: &str,
        since: Option<u64>,
    ) -> (broadcast::Receiver<PendingEvent>, Vec<PendingEvent>, Vec<TaskSnapshot>) {
        let mut state = self.inner.write().await;
        let receiver = state.worker_sender(worker_id).subscribe();
        let backlog = state
            .pending_events
            .get(worker_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|event| event.acked_at.is_none())
            .filter(|event| since.is_none_or(|cursor| event.event_id > cursor))
            .collect::<Vec<_>>();
        let snapshots = state
            .tasks
            .values()
            .filter(|task| task.target_worker_id == worker_id && !task.state.is_terminal())
            .map(TaskSnapshot::from)
            .collect::<Vec<_>>();
        (receiver, backlog, snapshots)
    }

    pub(crate) async fn verify_notify(&self, worker_id: &str, request: VerifyNotifyRequest) -> Result<(), String> {
        let mut state = self.inner.write().await;
        let events = state.pending_events.entry(worker_id.to_string()).or_default();
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

    pub(crate) async fn current_task_for_worker(&self, worker_id: &str) -> Option<ExecutionTask> {
        let state = self.inner.read().await;
        state
            .tasks
            .values()
            .filter(|task| task.target_worker_id == worker_id && !task.state.is_terminal())
            .max_by_key(|task| task.updated_at)
            .cloned()
    }

    pub(crate) async fn complete_task_with_success(
        &self,
        worker_id: &str,
        request_id: String,
        output: Value,
        state_patch: Value,
        agv_depart_payload: Option<Value>,
    ) -> Result<(), String> {
        let task = self
            .current_task_for_worker(worker_id)
            .await
            .ok_or_else(|| "no active task for worker".to_string())?;
        self.transition_task_success(&task.execution_id, output, state_patch)
            .await?;
        if let Some(payload) = agv_depart_payload {
            self.queue_pending_event(worker_id, Some(task.execution_id), "AGV_DEPART", payload)
                .await;
        }
        info!(worker_id = %worker_id, request_id = %request_id, "worker completed WCS task");
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
            &task.target_worker_id,
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
            &task.target_worker_id,
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

    pub(crate) async fn authenticated_worker_id(&self, headers: &axum::http::HeaderMap) -> Result<String, String> {
        let token = bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
        let state = self.inner.read().await;
        state
            .tokens
            .get(&token)
            .cloned()
            .ok_or_else(|| "invalid bearer token".to_string())
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
}

#[derive(Default)]
pub(crate) struct BridgeState {
    pub(crate) tasks: HashMap<String, ExecutionTask>,
    pub(crate) task_keys: HashMap<String, String>,
    pub(crate) tokens: HashMap<String, String>,
    pub(crate) worker_streams: HashMap<String, broadcast::Sender<PendingEvent>>,
    pub(crate) pending_events: HashMap<String, Vec<PendingEvent>>,
}

impl BridgeState {
    fn worker_sender(&mut self, worker_id: &str) -> broadcast::Sender<PendingEvent> {
        self.worker_streams
            .entry(worker_id.to_string())
            .or_insert_with(|| broadcast::channel(128).0)
            .clone()
    }
}

pub(crate) fn worker_id_from_connect(request: &ConnectRequest) -> String {
    request
        .station_id
        .clone()
        .or_else(|| request.station_ids.first().cloned())
        .or_else(|| request.client_id.clone())
        .unwrap_or_else(|| DEFAULT_CONNECT_WORKER_ID.to_string())
}

fn resolve_worker_id(request: &ExecuteRequest) -> Option<String> {
    value_string(&request.config, &["workerId", "stationId", "targetWorkerId"])
        .or_else(|| value_string(&request.context.input, &["workerId", "stationId", "targetWorkerId"]))
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
