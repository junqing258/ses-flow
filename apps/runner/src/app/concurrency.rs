use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

use super::app::AppError;
use crate::error::RunnerError;

const DEFAULT_MAX_GLOBAL: usize = 50;
const DEFAULT_QUEUE_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_PER_WORKFLOW: usize = 5;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverflowPolicy {
    #[default]
    Queue,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PerWorkflowConcurrencyConfig {
    pub default_max: usize,
    #[serde(flatten)]
    pub overrides: HashMap<String, usize>,
}

impl Default for PerWorkflowConcurrencyConfig {
    fn default() -> Self {
        Self {
            default_max: DEFAULT_MAX_PER_WORKFLOW,
            overrides: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ConcurrencyConfig {
    pub max_global: usize,
    pub queue_timeout_secs: u64,
    pub overflow_policy: OverflowPolicy,
    pub per_workflow: PerWorkflowConcurrencyConfig,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            max_global: DEFAULT_MAX_GLOBAL,
            queue_timeout_secs: DEFAULT_QUEUE_TIMEOUT_SECS,
            overflow_policy: OverflowPolicy::Queue,
            per_workflow: PerWorkflowConcurrencyConfig::default(),
        }
    }
}

impl ConcurrencyConfig {
    pub fn normalized(mut self) -> Self {
        self.max_global = self.max_global.max(1);
        self.per_workflow.default_max = self.per_workflow.default_max.max(1);
        self.per_workflow
            .overrides
            .values_mut()
            .for_each(|limit| *limit = (*limit).max(1));
        self
    }

    pub fn max_for_workflow(&self, workflow_key: &str) -> usize {
        self.per_workflow
            .overrides
            .get(workflow_key)
            .copied()
            .unwrap_or(self.per_workflow.default_max)
            .max(1)
    }
}

pub struct ConcurrencyGate {
    per_workflow: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    global: Arc<Semaphore>,
    config: ConcurrencyConfig,
}

impl ConcurrencyGate {
    pub fn new(config: ConcurrencyConfig) -> Self {
        let config = config.normalized();
        Self {
            per_workflow: Arc::new(Mutex::new(HashMap::new())),
            global: Arc::new(Semaphore::new(config.max_global)),
            config,
        }
    }

    pub fn config(&self) -> &ConcurrencyConfig {
        &self.config
    }

    pub async fn acquire(&self, workflow_key: &str) -> Result<ConcurrencyPermit, AppError> {
        let workflow_key = workflow_key.trim();
        let workflow_semaphore = self.workflow_semaphore(workflow_key);

        match self.config.overflow_policy {
            OverflowPolicy::Reject => self.try_acquire(workflow_key, workflow_semaphore),
            OverflowPolicy::Queue => self.acquire_with_queue(workflow_key, workflow_semaphore).await,
        }
    }

    fn workflow_semaphore(&self, workflow_key: &str) -> Arc<Semaphore> {
        let workflow_key = workflow_key.trim();
        let mut state = self
            .per_workflow
            .lock()
            .expect("concurrency gate workflow semaphore mutex poisoned");

        state
            .entry(workflow_key.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(self.config.max_for_workflow(workflow_key))))
            .clone()
    }

    fn try_acquire(
        &self,
        workflow_key: &str,
        workflow_semaphore: Arc<Semaphore>,
    ) -> Result<ConcurrencyPermit, AppError> {
        let workflow = workflow_semaphore.clone().try_acquire_owned().map_err(|error| {
            self.try_acquire_error(
                workflow_key,
                error,
                format!("workflow concurrency limit reached for {workflow_key}"),
            )
        })?;
        let global = self.global.clone().try_acquire_owned().map_err(|error| {
            self.try_acquire_error(
                workflow_key,
                error,
                format!("global workflow concurrency limit reached while starting {workflow_key}"),
            )
        })?;

        Ok(ConcurrencyPermit {
            _workflow: workflow,
            _global: global,
        })
    }

    async fn acquire_with_queue(
        &self,
        workflow_key: &str,
        workflow_semaphore: Arc<Semaphore>,
    ) -> Result<ConcurrencyPermit, AppError> {
        let started_at = Instant::now();
        let timeout_message = || {
            format!(
                "workflow concurrency queue timed out for {workflow_key} after {}s",
                self.config.queue_timeout_secs
            )
        };
        let workflow = self
            .acquire_owned(workflow_semaphore, started_at, &timeout_message)
            .await?;
        let global = self
            .acquire_owned(self.global.clone(), started_at, &timeout_message)
            .await?;

        Ok(ConcurrencyPermit {
            _workflow: workflow,
            _global: global,
        })
    }

    async fn acquire_owned(
        &self,
        semaphore: Arc<Semaphore>,
        started_at: Instant,
        timeout_message: &impl Fn() -> String,
    ) -> Result<OwnedSemaphorePermit, AppError> {
        match self.remaining_timeout(started_at) {
            Some(duration) if duration.is_zero() => Err(AppError::QueueTimeout(timeout_message())),
            Some(duration) => match tokio::time::timeout(duration, semaphore.acquire_owned()).await {
                Ok(Ok(permit)) => Ok(permit),
                Ok(Err(_)) => Err(AppError::Runner(RunnerError::Store(
                    "workflow concurrency gate is closed".to_string(),
                ))),
                Err(_) => Err(AppError::QueueTimeout(timeout_message())),
            },
            None => semaphore
                .acquire_owned()
                .await
                .map_err(|_| AppError::Runner(RunnerError::Store("workflow concurrency gate is closed".to_string()))),
        }
    }

    fn remaining_timeout(&self, started_at: Instant) -> Option<Duration> {
        (self.config.queue_timeout_secs != 0)
            .then_some(Duration::from_secs(self.config.queue_timeout_secs).saturating_sub(started_at.elapsed()))
    }

    fn try_acquire_error(&self, workflow_key: &str, error: TryAcquireError, message: String) -> AppError {
        match error {
            TryAcquireError::NoPermits => AppError::Throttled(message),
            TryAcquireError::Closed => AppError::Runner(RunnerError::Store(format!(
                "workflow concurrency gate is closed for {workflow_key}"
            ))),
        }
    }
}

pub struct ConcurrencyPermit {
    _workflow: OwnedSemaphorePermit,
    _global: OwnedSemaphorePermit,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Instant;

    use tokio::time::{Duration, sleep};

    use super::{ConcurrencyConfig, ConcurrencyGate, OverflowPolicy, PerWorkflowConcurrencyConfig};
    use crate::app::AppError;

    fn test_config(policy: OverflowPolicy) -> ConcurrencyConfig {
        ConcurrencyConfig {
            max_global: 1,
            queue_timeout_secs: 1,
            overflow_policy: policy,
            per_workflow: PerWorkflowConcurrencyConfig {
                default_max: 1,
                overrides: Default::default(),
            },
        }
    }

    #[tokio::test]
    async fn rejects_when_concurrency_limit_is_reached() {
        let gate = ConcurrencyGate::new(test_config(OverflowPolicy::Reject));
        let _permit = gate.acquire("reject-flow").await.expect("first acquire should succeed");

        let result = gate.acquire("reject-flow").await;

        assert!(matches!(result, Err(AppError::Throttled(_))));
    }

    #[tokio::test]
    async fn queues_until_slot_is_released() {
        let gate = Arc::new(ConcurrencyGate::new(test_config(OverflowPolicy::Queue)));
        let first = gate.acquire("queue-flow").await.expect("first acquire should succeed");
        let delayed_gate = gate.clone();

        let waiter = tokio::spawn(async move {
            let started_at = Instant::now();
            let permit = delayed_gate
                .acquire("queue-flow")
                .await
                .expect("second acquire should eventually succeed");
            (started_at.elapsed(), permit)
        });

        sleep(Duration::from_millis(150)).await;
        drop(first);

        let (elapsed, _permit) = waiter.await.expect("queue waiter should finish");
        assert!(elapsed >= Duration::from_millis(100));
    }
}
