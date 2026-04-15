use serde_json::json;

use super::template::{EvaluationContext, merge_state, nested_state_patch};

#[test]
fn resolves_exact_template_to_original_value_type() {
    let ctx = EvaluationContext {
        trigger: &json!({"body": {"orderNo": "SO-1"}}),
        input: &json!({}),
        state: &json!({}),
        env: json!({"warehouseId": "WH-1"}),
        output: &json!({}),
    };

    assert_eq!(ctx.resolve_value(&json!("{{trigger.body.orderNo}}")), json!("SO-1"));
    assert_eq!(ctx.resolve_value(&json!("{{env.warehouseId}}")), json!("WH-1"));
}

#[test]
fn merges_nested_state_patch_recursively() {
    let mut state = json!({
        "orderSnapshot": {
            "status": "created"
        }
    });

    merge_state(
        &mut state,
        nested_state_patch("orderSnapshot", json!({"status": "updated", "orderNo": "SO-1"})),
    );

    assert_eq!(
        state,
        json!({
            "orderSnapshot": {
                "status": "updated",
                "orderNo": "SO-1"
            }
        })
    );
}
