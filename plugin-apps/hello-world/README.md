# Hello World Plugin

这是一个按照 `docs/plans/dynamic-node-registry.md` 初始化的最小 HTTP 插件应用，放在 `plugin-apps/hello-world` 下，使用 Rust + Axum 实现。

当前实现聚焦首期能力：

- `transport = "http"`
- 同步 `POST /execute`
- `GET /descriptor` / `GET /health`
- `POST /cancel` / `POST /resume` 预留为占位接口，默认返回 `501 Not Implemented`

## 目录

```text
plugin-apps/hello-world
├── Cargo.toml
├── README.md
└── src
    ├── lib.rs
    └── main.rs
```

## 启动

在仓库根目录执行：

```bash
cargo run -p hello-world-plugin -- --host 127.0.0.1 --port 9101
```

## 接口

### `GET /descriptor`

返回插件描述符，关键字段：

- `id = "hello_world"`
- `runnerType = "plugin:hello_world"`
- `transport = "http"`
- `kind = "effect"`

### `GET /health`

健康检查，返回：

```json
{
  "status": "ok",
  "pluginId": "hello_world",
  "version": "0.1.0"
}
```

### `POST /execute`

请求体兼容当前 runner 已实现的 HTTP 插件协议：

```json
{
  "nodeId": "node-hello-1",
  "config": {
    "target": "World",
    "prefix": "Hello"
  },
  "context": {
    "runId": "run-1",
    "requestId": "req-1",
    "traceId": "trace-1",
    "workflowKey": "wf-hello",
    "workflowVersion": 1,
    "input": {
      "name": "SES"
    },
    "state": {},
    "env": {}
  }
}
```

示例响应：

```json
{
  "status": "success",
  "output": {
    "message": "Hello, SES!",
    "pluginId": "hello_world",
    "nodeId": "node-hello-1",
    "runId": "run-1",
    "requestId": "req-1",
    "traceId": "trace-1",
    "workflowKey": "wf-hello",
    "workflowVersion": 1,
    "receivedInput": {
      "name": "SES"
    },
    "receivedConfig": {
      "target": "World",
      "prefix": "Hello"
    }
  },
  "statePatch": {
    "plugins": {
      "hello_world": {
        "lastGreeting": "Hello, SES!"
      }
    }
  },
  "logs": [
    {
      "level": "info",
      "message": "hello-world executed for SES",
      "fields": {
        "pluginId": "hello_world"
      }
    }
  ]
}
```

## 本地验证

获取 descriptor：

```bash
curl http://127.0.0.1:9101/descriptor
```

执行插件：

```bash
curl -X POST http://127.0.0.1:9101/execute \
  -H 'Content-Type: application/json' \
  -d '{
    "nodeId":"node-hello-1",
    "config":{"prefix":"Hi"},
    "context":{
      "runId":"run-1",
      "requestId":"req-1",
      "traceId":"trace-1",
      "workflowKey":"wf-hello",
      "workflowVersion":1,
      "input":{"name":"SES"},
      "state":{},
      "env":{}
    }
  }'
```

## 注册到 backend

先启动 backend，再调用：

```bash
curl -X POST http://127.0.0.1:6302/runner-api/plugin-registrations \
  -H 'Content-Type: application/json' \
  -d '{"baseUrl":"http://127.0.0.1:9101"}'
```

注册成功后，backend 会回拉 `GET /descriptor` 并把该插件节点注册到 node registry。
