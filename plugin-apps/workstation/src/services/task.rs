use chrono::{Duration, Utc};
use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::{DEFAULT_RUNNER_RESUME_SIGNAL, normalize_runner_base_url};
use crate::models::{CancelRequest, ExecuteRequest, ExecutionTask, ResumeRequest, TaskErrorPayload, TaskState};
use crate::views::{failure_resume_event, success_resume_event};

use super::AppState;
use super::util::{task_lookup_key, value_string};

impl AppState {
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

    pub(crate) async fn current_robot_departure_task_for_worker(&self, station_id: &str) -> Option<ExecutionTask> {
        let state = self.inner.read().await;
        state
            .tasks
            .values()
            .filter(|task| {
                task.target_station_id == station_id
                    && !task.state.is_terminal()
                    && task.plugin_type == "plugin:robot_departure"
            })
            .max_by_key(|task| task.updated_at)
            .cloned()
    }

    pub(crate) async fn robot_departure_wait_task_id_for_worker(
        &self,
        station_id: &str,
        request_id: &str,
    ) -> Option<String> {
        let Some(db_pool) = self.db_pool.as_ref() else {
            warn!(station_id = %station_id, request_id = %request_id, "robot departure task lookup skipped because DATABASE_URL is not configured");
            return None;
        };

        let row = match sqlx::query(
            r#"
            SELECT execution.payload->>'taskId' AS task_id
            FROM workflow_runs
            CROSS JOIN jsonb_each(state->'workstation'->'executions') AS execution(id, payload)
            WHERE status IN ($1, $2)
              AND current_node_id = $3
              AND last_signal->>'type' = $4
              AND execution.payload->>'workerId' = $5
              AND execution.payload->>'taskId' IS NOT NULL
              AND (
                workflow_runs.request_id = $6
                OR last_signal->'payload'->>'requestId' = $6
                OR state->>'requestId' = $6
              )
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
        )
        .bind("\"waiting\"")
        .bind("waiting")
        .bind("robot_departure")
        .bind(DEFAULT_RUNNER_RESUME_SIGNAL)
        .bind(station_id)
        .bind(request_id)
        .fetch_optional(db_pool)
        .await
        {
            Ok(row) => row,
            Err(error) => {
                warn!(station_id = %station_id, request_id = %request_id, error = %error, "failed to look up robot departure task id from workflow context");
                return None;
            }
        };

        row.and_then(|row| row.try_get::<String, _>("task_id").ok())
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
}

#[derive(Debug, Clone)]
pub(crate) struct PendingRobotDeparture {
    pub(crate) station_id: String,
    pub(crate) task_id: String,
    pub(crate) agv_id: String,
    pub(crate) completed: i64,
    pub(crate) request_id: String,
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
