use std::sync::atomic::Ordering;

use chrono::Utc;
use serde_json::Value;

use crate::models::PendingEvent;

use super::AppState;

impl AppState {
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
}
