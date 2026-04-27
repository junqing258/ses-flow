use serde_json::json;

use crate::config::DEFAULT_RUNNER_RESUME_SIGNAL;
use crate::models::PluginDescriptor;

pub(crate) fn plugin_descriptors() -> Vec<PluginDescriptor> {
    vec![
        build_scan_task_descriptor(),
        build_get_task_info_descriptor(),
        build_robot_departure_descriptor(),
        build_pack_task_descriptor(),
    ]
}

fn build_scan_task_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "scan_task".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:scan_task".to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: "等待扫码".to_string(),
        description: "下发扫码任务到 App，挂起等待工人扫码；App 调用 scanBarcode 接口后携带 itemId 触发 resume".to_string(),
        color: Some("#F97316".to_string()),
        icon: Some("scan-barcode".to_string()),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 0,
        supports_cancel: true,
        supports_resume: true,
        config_schema: json!({
            "type": "object",
            "required": ["stationId"],
            "properties": {
                "stationId": {
                    "type": "string",
                    "title": "工作站 ID",
                    "description": "RCS 地图站点 ID；Bridge 以此作为 targetWorkerId 派发扫码任务"
                },
                "timeoutMs": {
                    "type": "integer",
                    "title": "扫码超时（ms）",
                    "default": 0,
                    "description": "0 表示不超时"
                },
                "runnerBaseUrl": {
                    "type": "string",
                    "title": "Runner API Base URL"
                },
                "waitSignalType": {
                    "type": "string",
                    "title": "恢复信号类型",
                    "default": DEFAULT_RUNNER_RESUME_SIGNAL
                }
            }
        }),
        defaults: json!({
            "timeoutMs": 0,
            "waitSignalType": DEFAULT_RUNNER_RESUME_SIGNAL
        }),
        input_schema: json!({
            "type": "object",
            "required": ["agvId"],
            "properties": {
                "agvId": {
                    "type": "string",
                    "title": "AGV 编号",
                    "description": "当前停靠工作站的小车 ID，由上游 wait(car_arrived) resume 写入 state"
                }
            }
        }),
        output_schema: json!({
            "type": "object",
            "required": ["itemId"],
            "properties": {
                "itemId": {
                    "type": "string",
                    "title": "商品唯一 ID",
                    "description": "scanBarcode 接口返回的 itemId，由 resume 携带写入 state"
                }
            }
        }),
        input_mapping_schema: json!({
            "type": "object"
        }),
        output_mapping_schema: json!({
            "type": "object"
        }),
    }
}

fn build_get_task_info_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "get_task_info".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:get_task_info".to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: "获取任务/订单信息".to_string(),
        description: "串联 getTaskInfo → lockTask，锁定库存并获取 taskId；结果通过 statePatch 写回 run state".to_string(),
        color: Some("#6366F1".to_string()),
        icon: Some("clipboard-list".to_string()),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 10_000,
        supports_cancel: false,
        supports_resume: false,
        config_schema: json!({
            "type": "object",
            "required": ["sesBaseUrl"],
            "properties": {
                "sesBaseUrl": {
                    "type": "string",
                    "title": "SES API Base URL",
                    "description": "工作站接口根路径，如 http://ses-host/station/operation"
                },
                "timeoutMs": {
                    "type": "integer",
                    "title": "请求超时（ms）",
                    "default": 10000
                }
            }
        }),
        defaults: json!({
            "timeoutMs": 10000
        }),
        input_schema: json!({
            "type": "object",
            "required": ["stationId", "itemId"],
            "properties": {
                "stationId": {
                    "type": "string",
                    "title": "工作站 ID",
                    "description": "来自 state.stationId，由上游波次流程写入"
                },
                "itemId": {
                    "type": "string",
                    "title": "商品唯一 ID",
                    "description": "来自 state.itemId，由 plugin:scan_task resume 写入"
                }
            }
        }),
        output_schema: json!({
            "type": "object",
            "required": ["taskId", "orderId", "orderDetailId", "targetId", "count"],
            "properties": {
                "taskId": {
                    "type": "string",
                    "title": "任务 ID",
                    "description": "lockTask 返回，用于后续 robotDeparture"
                },
                "orderId": { "type": "string", "title": "订单 ID" },
                "orderDetailId": { "type": "string", "title": "订单明细 ID" },
                "targetId": { "type": "string", "title": "目的地 ID（格口）" },
                "count": { "type": "integer", "title": "本次发车件数" }
            }
        }),
        input_mapping_schema: json!({
            "type": "object"
        }),
        output_mapping_schema: json!({
            "type": "object"
        }),
    }
}

fn build_robot_departure_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "robot_departure".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:robot_departure".to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: "发车".to_string(),
        description: "调用 robotDeparture 接口通知 RCS 小车离站；result 枚举：SUCCESS(0) / NO_AGV(1) / NO_TASK(2)".to_string(),
        color: Some("#10B981".to_string()),
        icon: Some("truck".to_string()),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 10_000,
        supports_cancel: false,
        supports_resume: false,
        config_schema: json!({
            "type": "object",
            "required": ["sesBaseUrl"],
            "properties": {
                "sesBaseUrl": {
                    "type": "string",
                    "title": "SES API Base URL",
                    "description": "工作站接口根路径，如 http://ses-host/station/operation"
                },
                "timeoutMs": {
                    "type": "integer",
                    "title": "请求超时（ms）",
                    "default": 10000
                }
            }
        }),
        defaults: json!({
            "timeoutMs": 10000
        }),
        input_schema: json!({
            "type": "object",
            "required": ["taskId"],
            "properties": {
                "taskId": {
                    "type": "string",
                    "title": "任务 ID",
                    "description": "来自 state.taskId，由 plugin:get_task_info statePatch 写入"
                }
            }
        }),
        output_schema: json!({
            "type": "object",
            "required": ["result"],
            "properties": {
                "result": {
                    "type": "integer",
                    "title": "发车结果",
                    "description": "0=SUCCESS，1=NO_AGV（无小车），2=NO_TASK（任务不存在）",
                    "enum": [0, 1, 2]
                }
            }
        }),
        input_mapping_schema: json!({
            "type": "object"
        }),
        output_mapping_schema: json!({
            "type": "object"
        }),
    }
}

fn build_pack_task_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "pack_task".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:pack_task".to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: "集包确认".to_string(),
        description: "人工工作台集包确认桥接节点".to_string(),
        color: Some("#14B8A6".to_string()),
        icon: Some("badge-check".to_string()),
        status: "stable".to_string(),
        transport: "http".to_string(),
        timeout_ms: 0,
        supports_cancel: true,
        supports_resume: false,
        config_schema: json!({
            "type": "object",
            "required": ["stationId"],
            "properties": {
                "stationId": {
                    "type": "string",
                    "title": "工作站 ID",
                    "description": "执行集包操作的工作站；Bridge 以此作为 targetWorkerId 派发任务"
                },
                "timeoutMs": {
                    "type": "integer",
                    "title": "集包确认超时（ms）",
                    "default": 0
                },
                "runnerBaseUrl": {
                    "type": "string",
                    "title": "Runner API Base URL"
                },
                "waitSignalType": {
                    "type": "string",
                    "title": "恢复信号类型",
                    "default": DEFAULT_RUNNER_RESUME_SIGNAL
                }
            }
        }),
        defaults: json!({
            "timeoutMs": 0,
            "waitSignalType": DEFAULT_RUNNER_RESUME_SIGNAL
        }),
        input_schema: json!({
            "type": "object",
            "required": ["chuteId", "waveId", "itemCount"],
            "properties": {
                "chuteId": { "type": "string", "title": "需集包的格口 ID" },
                "waveId": { "type": "string", "title": "所属波次 ID" },
                "itemCount": { "type": "integer", "title": "格口当前件数" }
            }
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "packId": { "type": "string", "title": "集包单号（操作员扫码或系统生成）" },
                "confirmedCount": { "type": "integer", "title": "操作员确认的实际件数" }
            }
        }),
        input_mapping_schema: json!({
            "type": "object"
        }),
        output_mapping_schema: json!({
            "type": "object"
        }),
    }
}
