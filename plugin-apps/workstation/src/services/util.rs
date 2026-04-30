use serde_json::Value;

pub(super) fn task_lookup_key(run_id: &str, node_id: &str, request_id: &str) -> String {
    format!("{run_id}:{node_id}:{request_id}")
}

pub(super) fn value_string(value: &Value, candidates: &[&str]) -> Option<String> {
    candidates.iter().find_map(|key| {
        value.get(key).and_then(Value::as_str).map(str::to_string).or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
    })
}

pub(super) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}
