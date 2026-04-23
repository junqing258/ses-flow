// 简单测试：演示节点输入输出日志功能
// 运行方式: RUNNER_LOG=runner=info cargo run --example simple_test

use serde_json::json;

fn main() {
    // 初始化日志系统
    let _log_guard = runner::utils::telemetry::init_tracing();

    println!("\n========================================");
    println!("工作流节点输入输出日志演示");
    println!("========================================\n");

    // 使用JSON定义工作流
    let workflow_json = json!({
        "meta": {
            "key": "demo-workflow",
            "name": "演示工作流",
            "version": 1,
            "scope": {
                "tenant": "tenant-a",
                "warehouse": "WH-1"
            }
        },
        "trigger": {
            "type": "manual"
        },
        "inputSchema": {
            "type": "object"
        },
        "nodes": [
            {
                "id": "start_1",
                "type": "start",
                "name": "开始"
            },
            {
                "id": "process_1",
                "type": "set_state",
                "name": "处理数据",
                "config": {
                    "path": "result"
                },
                "inputMapping": {
                    "value": {
                        "userId": "{{trigger.body.userId}}",
                        "processed": true,
                        "timestamp": "2024-01-01T00:00:00Z"
                    }
                }
            },
            {
                "id": "end_1",
                "type": "end",
                "name": "结束"
            }
        ],
        "transitions": [
            {
                "from": "start_1",
                "to": "process_1"
            },
            {
                "from": "process_1",
                "to": "end_1"
            }
        ],
        "policies": {}
    });

    // 解析工作流定义
    let workflow: runner::core::definition::WorkflowDefinition =
        serde_json::from_value(workflow_json).expect("工作流定义应该有效");

    // 创建引擎
    let engine = runner::core::engine::WorkflowEngine::new();

    println!("开始运行工作流...");
    println!("提示: 观察下方日志中的 'node input before execution' 和 'node output after execution'");
    println!();

    // 运行工作流
    let result = engine.run(
        &workflow,
        json!({
            "body": {
                "userId": "user-001"
            }
        }),
        runner::core::runtime::RunEnvironment::default(),
    );

    println!("\n========================================");
    match result {
        Ok(summary) => {
            println!("✓ 工作流执行成功");
            println!("  状态: {:?}", summary.status);
            println!("  当前节点: {:?}", summary.current_node_id);
            if let Ok(state_str) = serde_json::to_string_pretty(&summary.state) {
                println!("  状态数据:\n{}", state_str);
            }
        }
        Err(e) => {
            println!("✗ 工作流执行失败: {}", e);
        }
    }
    println!("========================================\n");

    println!("提示: 查看上方的日志输出，可以看到每个节点的输入和输出数据\n");
}
