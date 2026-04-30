//! Runner 恢复与等待流程查询。
//! 负责调用 runner resume API，并从数据库查找可被工位事件唤醒的等待运行。

use serde_json::{Value, json};
use sqlx::Row;
use tracing::{info, warn};

use crate::config::DEFAULT_RUNNER_RESUME_SIGNAL;
use crate::models::ExecutionTask;

use super::AppState;

impl AppState {
    pub(super) async fn resume_runner(&self, task: &ExecutionTask, event: Value) -> Result<(), String> {
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

    pub(super) async fn resume_agv_arrival_waits(
        &self,
        station_id: &str,
        agv_id: &str,
        request_id: Option<&str>,
    ) -> Vec<String> {
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
              AND EXISTS (
                SELECT 1
                FROM jsonb_each(state->'workstation'->'executions') AS execution(id, payload)
                WHERE execution.payload->>'taskId' = $5
                  AND execution.payload->>'workerId' = $6
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
