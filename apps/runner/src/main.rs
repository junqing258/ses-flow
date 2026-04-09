use std::env;
use std::fs;

use runner::definition::WorkflowDefinition;
use runner::engine::WorkflowEngine;
use runner::runtime::{RunEnvironment, WorkflowRunSnapshot, WorkflowRunSummary};
use serde_json::{Value, json};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let workflow_path = parse_arg("--workflow");
    let trigger_path = parse_arg("--trigger");
    let resume_state_path = parse_arg("--resume-state");
    let event_path = parse_arg("--event");

    let definition: WorkflowDefinition = match workflow_path {
        Some(path) => serde_json::from_str(&fs::read_to_string(path)?)?,
        None => serde_json::from_str(include_str!("../examples/sorting-main-flow.json"))?,
    };

    let engine = WorkflowEngine::new();
    let summary = match resume_state_path {
        Some(path) => {
            let snapshot = load_resume_state(&path)?;
            let event = match event_path {
                Some(path) => serde_json::from_str::<Value>(&fs::read_to_string(path)?)?,
                None => default_resume_event(),
            };
            engine.resume(&definition, snapshot, event)?
        }
        None => {
            let trigger = match trigger_path {
                Some(path) => serde_json::from_str::<Value>(&fs::read_to_string(path)?)?,
                None => default_trigger(),
            };
            engine.run(&definition, trigger, RunEnvironment::default())?
        }
    };

    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn parse_arg(flag: &str) -> Option<String> {
    let args = env::args().collect::<Vec<_>>();
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn default_trigger() -> Value {
    json!({
        "headers": {
            "requestId": "req-demo-1"
        },
        "body": {
            "orderNo": "SO-DEMO-1",
            "bizType": "auto_sort"
        }
    })
}

fn default_resume_event() -> Value {
    json!({
        "event": "rcs.callback",
        "status": "done",
        "orderNo": "SO-DEMO-1"
    })
}

fn load_resume_state(path: &str) -> Result<WorkflowRunSnapshot, Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(path)?;
    let value = serde_json::from_str::<Value>(&raw)?;

    if value.get("resumeState").is_some() {
        let summary = serde_json::from_value::<WorkflowRunSummary>(value)?;
        return summary
            .resume_state
            .ok_or_else(|| "resumeState is missing from workflow summary".into());
    }

    Ok(serde_json::from_str::<WorkflowRunSnapshot>(&raw)?)
}
