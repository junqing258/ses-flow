use serde::Serialize;
use serde_json::{Map, Value, json};

#[derive(Debug)]
pub struct EvaluationContext<'a> {
    pub trigger: &'a Value,
    pub input: &'a Value,
    pub state: &'a Value,
    pub env: &'a Value,
    pub output: &'a Value,
}

impl<'a> EvaluationContext<'a> {
    pub fn resolve_value(&self, value: &Value) -> Value {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => value.clone(),
            Value::String(text) => self.resolve_string(text),
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| self.resolve_value(item))
                    .collect::<Vec<_>>(),
            ),
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), self.resolve_value(value)))
                    .collect::<Map<_, _>>(),
            ),
        }
    }

    pub fn resolve_path(&self, path: &str) -> Option<Value> {
        let mut segments = path.split('.');
        let root = segments.next()?;

        let mut current = match root {
            "trigger" => self.trigger,
            "input" => self.input,
            "state" => self.state,
            "env" => self.env,
            "output" => self.output,
            _ => return None,
        };

        for segment in segments {
            current = current.get(segment)?;
        }

        Some(current.clone())
    }

    fn resolve_string(&self, text: &str) -> Value {
        if let Some(path) = exact_template_path(text) {
            return self.resolve_path(path).unwrap_or(Value::Null);
        }

        if !text.contains("{{") {
            return Value::String(text.to_string());
        }

        let mut rendered = String::new();
        let mut cursor = 0usize;

        while let Some(start) = text[cursor..].find("{{") {
            let start_index = cursor + start;
            rendered.push_str(&text[cursor..start_index]);

            if let Some(end) = text[start_index + 2..].find("}}") {
                let end_index = start_index + 2 + end;
                let path = text[start_index + 2..end_index].trim();
                let replacement = self
                    .resolve_path(path)
                    .unwrap_or(Value::Null)
                    .to_string()
                    .trim_matches('"')
                    .to_string();
                rendered.push_str(&replacement);
                cursor = end_index + 2;
            } else {
                rendered.push_str(&text[start_index..]);
                cursor = text.len();
                break;
            }
        }

        if cursor < text.len() {
            rendered.push_str(&text[cursor..]);
        }

        Value::String(rendered)
    }
}

pub fn env_to_value<T>(env: &T) -> Value
where
    T: Serialize,
{
    serde_json::to_value(env).unwrap_or_else(|_| json!({}))
}

pub fn nested_state_patch(path: &str, value: Value) -> Value {
    let segments = path
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return value;
    }

    let mut acc = value;
    for segment in segments.into_iter().rev() {
        acc = Value::Object(Map::from_iter([(segment.to_string(), acc)]));
    }
    acc
}

pub fn merge_state(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                match target_map.get_mut(&key) {
                    Some(existing) => merge_state(existing, value),
                    None => {
                        target_map.insert(key, value);
                    }
                }
            }
        }
        (target_slot, patch_value) => {
            *target_slot = patch_value;
        }
    }
}

fn exact_template_path(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    if !trimmed.starts_with("{{") || !trimmed.ends_with("}}") {
        return None;
    }

    let inner = trimmed
        .strip_prefix("{{")
        .and_then(|text| text.strip_suffix("}}"))?
        .trim();

    if inner.is_empty() {
        return None;
    }

    Some(inner)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{EvaluationContext, merge_state, nested_state_patch};

    #[test]
    fn resolves_exact_template_to_original_value_type() {
        let ctx = EvaluationContext {
            trigger: &json!({"body": {"orderNo": "SO-1"}}),
            input: &json!({}),
            state: &json!({}),
            env: &json!({"warehouseId": "WH-1"}),
            output: &json!({}),
        };

        assert_eq!(
            ctx.resolve_value(&json!("{{trigger.body.orderNo}}")),
            json!("SO-1")
        );
        assert_eq!(
            ctx.resolve_value(&json!("{{env.warehouseId}}")),
            json!("WH-1")
        );
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
            nested_state_patch(
                "orderSnapshot",
                json!({"status": "updated", "orderNo": "SO-1"}),
            ),
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
}
