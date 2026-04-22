use serde_json::json;

use crate::core::definition::WorkflowDefinition;
use crate::services::WorkflowServices;

#[test]
fn workflow_definition_registry_round_trips_registered_definition() {
    let mut services = WorkflowServices::with_defaults();
    let definition: WorkflowDefinition = serde_json::from_value(json!({
        "meta": {
            "key": "workflow.demo",
            "version": 1
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": {
            "type": "object"
        },
        "nodes": [
            { "id": "start_1", "type": "start", "name": "Start" },
            { "id": "end_1", "type": "end", "name": "End" }
        ],
        "transitions": [
            { "from": "start_1", "to": "end_1" }
        ],
        "policies": {}
    }))
    .expect("workflow definition should deserialize");

    services
        .workflow_definitions
        .register("workflow.demo", definition.clone());

    assert_eq!(
        services
            .workflow_definitions
            .resolve("workflow.demo")
            .expect("registered definition should exist")
            .meta
            .key,
        definition.meta.key
    );
}
