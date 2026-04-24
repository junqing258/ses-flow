use serde_json::json;

use crate::core::definition::WorkflowDefinition;
use crate::services::{NodeDescriptor, NodeTransport, WorkflowServices};

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

#[test]
fn node_descriptor_registry_round_trips_registered_plugin_descriptor() {
    let mut services = WorkflowServices::with_defaults();
    services.node_descriptors.register(NodeDescriptor {
        id: "barcode_scan".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:barcode_scan".to_string(),
        version: "1.0.0".to_string(),
        category: "业务节点".to_string(),
        display_name: "条码扫描".to_string(),
        description: None,
        color: Some("#0EA5E9".to_string()),
        icon: None,
        status: Default::default(),
        required_permissions: Vec::new(),
        transport: Some(NodeTransport::Http),
        endpoint: Some("http://127.0.0.1:9001".to_string()),
        plugin_app_id: None,
        plugin_app_name: None,
        binary: None,
        timeout_ms: Some(5_000),
        supports_cancel: false,
        supports_resume: false,
        config_schema: json!({"type": "object"}),
        defaults: None,
        input_mapping_schema: None,
        output_mapping_schema: None,
    });

    let descriptor = services
        .node_descriptors
        .resolve_by_runner_type("plugin:barcode_scan")
        .expect("registered descriptor should exist");

    assert_eq!(descriptor.display_name, "条码扫描");
    assert_eq!(descriptor.endpoint.as_deref(), Some("http://127.0.0.1:9001"));
    assert_eq!(descriptor.color.as_deref(), Some("#0EA5E9"));
}
