use async_stream::stream;
use axum::response::sse::{Event, Sse};
use chrono::{DateTime, Utc};
use futures_core::Stream;
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

use crate::core::runtime::{WorkflowRunStatus, WorkflowRunSummary};
use crate::store::WorkflowEditSessionRecord;

const EVENT_CHANNEL_CAPACITY: usize = 32;
const ALL_WORKFLOWS_TOPIC: &str = "__all_workflows__";

pub type WorkflowEventStream = Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>;

#[derive(Debug, Clone, Serialize)]
pub struct WorkflowStreamNotification {
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(rename = "workflowId", skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(rename = "runId", skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<WorkflowRunStatus>,
    #[serde(rename = "missedEvents", skip_serializing_if = "Option::is_none")]
    pub missed_events: Option<u64>,
    #[serde(rename = "emittedAt")]
    pub emitted_at: DateTime<Utc>,
}

impl WorkflowStreamNotification {
    fn connected(workflow_id: Option<String>, run_id: Option<String>, session_id: Option<String>) -> Self {
        Self {
            event_type: "stream.connected".to_string(),
            workflow_id,
            run_id,
            session_id,
            status: None,
            missed_events: None,
            emitted_at: Utc::now(),
        }
    }

    fn resync_required(
        workflow_id: Option<String>,
        run_id: Option<String>,
        session_id: Option<String>,
        missed_events: u64,
    ) -> Self {
        Self {
            event_type: "stream.resync-required".to_string(),
            workflow_id,
            run_id,
            session_id,
            status: None,
            missed_events: Some(missed_events),
            emitted_at: Utc::now(),
        }
    }

    pub fn run_changed(summary: &WorkflowRunSummary, workflow_id: Option<&str>) -> Self {
        Self {
            event_type: "run.changed".to_string(),
            workflow_id: workflow_id.map(str::to_string),
            run_id: Some(summary.run_id.clone()),
            session_id: None,
            status: Some(summary.status.clone()),
            missed_events: None,
            emitted_at: Utc::now(),
        }
    }

    pub fn workflow_runs_changed(workflow_id: &str, summary: &WorkflowRunSummary) -> Self {
        Self {
            event_type: "workflow.runs.changed".to_string(),
            workflow_id: Some(workflow_id.to_string()),
            run_id: Some(summary.run_id.clone()),
            session_id: None,
            status: Some(summary.status.clone()),
            missed_events: None,
            emitted_at: Utc::now(),
        }
    }

    pub fn workflow_changed(workflow_id: &str) -> Self {
        Self {
            event_type: "workflow.changed".to_string(),
            workflow_id: Some(workflow_id.to_string()),
            run_id: None,
            session_id: None,
            status: None,
            missed_events: None,
            emitted_at: Utc::now(),
        }
    }

    pub fn session_changed(session: &WorkflowEditSessionRecord) -> Self {
        Self {
            event_type: "session.changed".to_string(),
            workflow_id: session.workflow_id.clone(),
            run_id: None,
            session_id: Some(session.session_id.clone()),
            status: None,
            missed_events: None,
            emitted_at: Utc::now(),
        }
    }
}

type TopicMap = Arc<Mutex<HashMap<String, broadcast::Sender<WorkflowStreamNotification>>>>;

#[derive(Clone, Default)]
pub struct WorkflowEventStreams {
    run_topics: TopicMap,
    session_topics: TopicMap,
    workflow_topics: TopicMap,
}

impl WorkflowEventStreams {
    pub fn subscribe_run(&self, run_id: &str) -> WorkflowEventStream {
        self.subscribe(
            &self.run_topics,
            run_id,
            WorkflowStreamNotification::connected(None, Some(run_id.to_string()), None),
        )
    }

    pub fn subscribe_session(&self, session_id: &str) -> WorkflowEventStream {
        self.subscribe(
            &self.session_topics,
            session_id,
            WorkflowStreamNotification::connected(None, None, Some(session_id.to_string())),
        )
    }

    pub fn subscribe_workflow(&self, workflow_id: &str) -> WorkflowEventStream {
        self.subscribe(
            &self.workflow_topics,
            workflow_id,
            WorkflowStreamNotification::connected(Some(workflow_id.to_string()), None, None),
        )
    }

    pub fn subscribe_workflows(&self) -> WorkflowEventStream {
        self.subscribe(
            &self.workflow_topics,
            ALL_WORKFLOWS_TOPIC,
            WorkflowStreamNotification::connected(None, None, None),
        )
    }

    pub fn publish_run_changed(&self, summary: &WorkflowRunSummary, workflow_id: Option<&str>) {
        let notification = WorkflowStreamNotification::run_changed(summary, workflow_id);
        self.publish(&self.run_topics, &summary.run_id, notification);
    }

    pub fn publish_workflow_runs_changed(&self, workflow_id: &str, summary: &WorkflowRunSummary) {
        let notification = WorkflowStreamNotification::workflow_runs_changed(workflow_id, summary);
        self.publish(&self.workflow_topics, workflow_id, notification);
        self.publish(
            &self.workflow_topics,
            ALL_WORKFLOWS_TOPIC,
            WorkflowStreamNotification::workflow_runs_changed(workflow_id, summary),
        );
    }

    pub fn publish_workflow_changed(&self, workflow_id: &str) {
        let notification = WorkflowStreamNotification::workflow_changed(workflow_id);
        self.publish(&self.workflow_topics, workflow_id, notification);
        self.publish(
            &self.workflow_topics,
            ALL_WORKFLOWS_TOPIC,
            WorkflowStreamNotification::workflow_changed(workflow_id),
        );
    }

    pub fn publish_session_changed(&self, session: &WorkflowEditSessionRecord) {
        let notification = WorkflowStreamNotification::session_changed(session);
        self.publish(&self.session_topics, &session.session_id, notification);
    }

    fn subscribe(
        &self,
        topics: &TopicMap,
        key: &str,
        initial_notification: WorkflowStreamNotification,
    ) -> WorkflowEventStream {
        let sender = self.ensure_sender(topics, key);
        let mut receiver = sender.subscribe();
        let reconnect_notification = initial_notification.clone();
        let stream = stream! {
            yield Ok(Self::to_event(initial_notification));

            loop {
                match receiver.recv().await {
                    Ok(notification) => {
                        yield Ok(Self::to_event(notification));
                    }
                    Err(broadcast::error::RecvError::Lagged(missed_events)) => {
                        yield Ok(Self::to_event(WorkflowStreamNotification::resync_required(
                            reconnect_notification.workflow_id.clone(),
                            reconnect_notification.run_id.clone(),
                            reconnect_notification.session_id.clone(),
                            missed_events,
                        )));
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        };

        Sse::new(Box::pin(stream))
    }

    fn publish(&self, topics: &TopicMap, key: &str, notification: WorkflowStreamNotification) {
        let sender = self.ensure_sender(topics, key);
        let _ = sender.send(notification);
    }

    fn ensure_sender(&self, topics: &TopicMap, key: &str) -> broadcast::Sender<WorkflowStreamNotification> {
        let mut state = topics.lock().expect("workflow event topics lock should be available");

        state
            .entry(key.to_string())
            .or_insert_with(|| {
                let (sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
                sender
            })
            .clone()
    }

    fn to_event(notification: WorkflowStreamNotification) -> Event {
        match Event::default()
            .event(notification.event_type.as_str())
            .json_data(&notification)
        {
            Ok(event) => event,
            Err(_) => Event::default().event("stream.serialization-error"),
        }
    }
}
