use serde_json::json;

use crate::config::DEFAULT_RUNNER_RESUME_SIGNAL;
use crate::models::PluginDescriptor;

pub(crate) fn plugin_descriptors() -> Vec<PluginDescriptor> {
    vec![build_scan_task_descriptor(), build_pack_task_descriptor()]
}

fn build_scan_task_descriptor() -> PluginDescriptor {
    PluginDescriptor {
        id: "scan_task".to_string(),
        kind: "effect".to_string(),
        runner_type: "plugin:scan_task".to_string(),
        version: "1.0.0".to_string(),
        category: "人工工作台".to_string(),
        display_name: "工作站扫码拣货".to_string(),
        description: "人工工作台扫码拣货桥接节点".to_string(),
        color: Some("#F97316".to_string()),
        icon: Some("package-check".to_string()),
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
                    "title": "工作站 ID（platformId）",
                    "description": "RCS 地图站点 ID；Bridge 以此作为 targetWorkerId 派发任务"
                },
                "waveType": {
                    "type": "string",
                    "title": "波次类型",
                    "enum": ["ORDER", "PICKING"],
                    "default": "ORDER",
                    "description": "透传给工作站 getTaskInfo 接口的 WaveType 字段"
                },
                "timeoutMs": {
                    "type": "integer",
                    "title": "任务超时（ms）",
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
            "waveType": "ORDER",
            "timeoutMs": 0,
            "waitSignalType": DEFAULT_RUNNER_RESUME_SIGNAL
        }),
        input_schema: json!({
            "type": "object",
            "required": ["orderId", "waveId", "barcode", "chuteId", "count"],
            "properties": {
                "orderId": { "type": "string", "title": "订单 ID" },
                "waveId": { "type": "string", "title": "波次 ID" },
                "sku": { "type": "string", "title": "商品 SKU（可选）" },
                "barcode": { "type": "string", "title": "期望扫描条码" },
                "chuteId": { "type": "string", "title": "目标格口 ID" },
                "count": { "type": "integer", "title": "本次需拣数量" },
                "lockId": {
                    "type": "string",
                    "title": "库存锁 ID（可选，透传给 getTaskInfo.LockId）"
                }
            }
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string", "title": "WCS 内部任务 ID（来自 getTaskInfo 响应）" },
                "scannedBarcode": { "type": "string", "title": "操作员实际扫描到的条码" },
                "agvId": { "type": "string", "title": "执行本次任务的 AGV 编号" },
                "completed": { "type": "integer", "title": "本次实际完成件数" },
                "chuteId": { "type": "string", "title": "WCS 确认的实际投放格口" }
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
