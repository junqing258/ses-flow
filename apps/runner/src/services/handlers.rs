use std::collections::HashMap;

use crate::core::definition::WorkflowDefinition;

#[derive(Default, Clone)]
pub struct WorkflowServices {
    pub workflow_definitions: WorkflowDefinitionRegistry,
}

impl WorkflowServices {
    pub fn with_defaults() -> Self {
        Self::default()
    }
}

#[derive(Default, Clone)]
pub struct WorkflowDefinitionRegistry {
    definitions: HashMap<String, WorkflowDefinition>,
}

impl WorkflowDefinitionRegistry {
    pub fn register(&mut self, key: impl Into<String>, definition: WorkflowDefinition) {
        self.definitions.insert(key.into(), definition);
    }

    pub fn resolve(&self, key: &str) -> Option<WorkflowDefinition> {
        self.definitions.get(key).cloned()
    }
}
