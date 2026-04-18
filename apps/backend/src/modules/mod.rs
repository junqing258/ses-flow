use std::convert::Infallible;
use std::pin::Pin;

use async_stream::stream;
use axum::response::sse::{Event, Sse};
use futures_core::Stream;
use futures_util::StreamExt;
use runner::app::WorkflowStreamNotification;

pub mod ai_gateway;
pub mod edit_session;
pub mod routes;
pub mod run;
pub mod system;
pub mod workflow;

pub use routes::{ApiError, ApiState, RUNNER_API_BASE_PATH, RUNNER_VIEWS_BASE_PATH, build_router};

pub type WorkflowEventStream = Sse<Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>;

pub fn into_sse(mut source: runner::app::WorkflowEventStream) -> WorkflowEventStream {
    let stream = stream! {
        while let Some(notification) = source.next().await {
            yield Ok(to_event(notification));
        }
    };

    Sse::new(Box::pin(stream))
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

#[cfg(test)]
mod tests;
