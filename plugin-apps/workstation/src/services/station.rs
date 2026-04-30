use std::sync::atomic::Ordering;

use chrono::Utc;
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tracing::info;

use crate::config::DEFAULT_CONNECT_STATION_ID;
use crate::models::{ConnectRequest, PendingEvent, TaskSnapshot, VerifyNotifyRequest};

use super::AppState;
use super::util::value_to_string;

impl AppState {
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
}

pub(crate) struct AgvArrivalSimulation {
    pub(crate) event: PendingEvent,
    pub(crate) resumed_run_ids: Vec<String>,
}

pub(crate) fn station_id_from_connect(request: &ConnectRequest) -> String {
    request
        .station_id
        .clone()
        .or_else(|| request.station_ids.first().cloned())
        .or_else(|| request.client_id.clone())
        .unwrap_or_else(|| DEFAULT_CONNECT_STATION_ID.to_string())
}
