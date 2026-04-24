# Hello World Plugin

这是一个最小 HTTP plugin-app 示例，放在 `plugin-apps/hello-world` 下，使用 Rust + Axum 实现。

当前代码已经按 MVC 风格拆分，适合作为后续新 plugin-app 的参考骨架。

当前实现聚焦首期能力：

- `transport = "http"`
- 一个 plugin-app 暴露多个节点定义
- 同步 `POST /execute`
- `GET /descriptors` / `GET /descriptor` / `GET /health`
- `POST /cancel` / `POST /resume` 预留为占位接口，默认返回 `501 Not Implemented`
- 控制器 / 服务 / 模型 / 视图辅助 已拆分到独立模块目录

## 目录

```text
plugin-apps/hello-world
├── Cargo.toml
├── README.md
└── src
    ├── controllers
    │   ├── mod.rs
    │   └── plugin.rs
    ├── models
    │   └── mod.rs
    ├── services
    │   └── mod.rs
    ├── lib.rs
    ├── main.rs
    ├── router.rs
    ├── tests.rs
    └── views.rs
```

模块职责：

- `controllers/`：处理 HTTP 请求与响应状态码
- `models/`：定义 descriptor、execute request/response 等结构
- `services/`：封装 descriptor 组装和执行逻辑
- `views.rs`：统一 JSON 响应与 `X-Trace-Id` header 回写
- `router.rs`：集中声明插件路由

## 启动

在仓库根目录执行：

```bash
cargo run -p hello-world-plugin -- --host 127.0.0.1 --port 9101
```

或使用仓库内置命令：

```bash
just dev-plugin-hello-world
```

## 接口

### `GET /descriptors`

返回插件描述符数组。当前 `hello-world` 一次暴露两个节点定义：

- `id = "hello_world"`，`runnerType = "plugin:hello_world"`
- `id = "hello_world_formal"`，`runnerType = "plugin:hello_world_formal"`

这也是 backend 新协议优先回拉的接口。

### `GET /descriptor`

兼容旧协议，返回第一个插件描述符，也就是 `hello_world`：

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
  "pluginId": "hello_world",
  "runnerType": "plugin:hello_world",
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

当一个 plugin-app 返回多个 descriptor 时，runner 会在执行请求里带上：

- `pluginId`：当前命中的 descriptor id
- `runnerType`：当前节点类型，用于插件内部路由到具体实现

示例响应：

```json
{
  "status": "success",
  "output": {
    "message": "Hello, SES!",
    "pluginId": "hello_world",
    "runnerType": "plugin:hello_world",
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
        "lastGreeting": "Hello, SES!",
        "lastRunId": "run-1",
        "lastRequestId": "req-1",
        "lastNodeId": "node-hello-1",
        "traceId": "trace-1",
        "inputEcho": {
          "name": "SES"
        }
      }
    }
  },
  "logs": [
    {
      "level": "info",
      "message": "hello-world executed for SES",
      "fields": {
        "pluginId": "hello_world",
        "runnerType": "plugin:hello_world",
        "nodeId": "node-hello-1",
        "workflowKey": "wf-hello"
      }
    }
  ]
}
```

如果请求里带了 `context.traceId`，插件会在响应头里原样回写 `X-Trace-Id`。

## 本地验证

获取 descriptor：

```bash
curl http://127.0.0.1:9101/descriptors
```

兼容旧协议：

```bash
curl http://127.0.0.1:9101/descriptor
```

执行插件：

```bash
curl -X POST http://127.0.0.1:9101/execute \
  -H 'Content-Type: application/json' \
  -d '{
    "pluginId":"hello_world",
    "runnerType":"plugin:hello_world",
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

注册成功后，backend 会优先回拉 `GET /descriptors`，把返回的多个节点都注册到 node registry；如果插件还只有旧协议，backend 会自动回退到 `GET /descriptor`。

如果希望 backend 启动时自动注册该插件，也可以直接设置：

```bash
BACKEND_AUTO_REGISTER_PLUGIN_URLS=http://127.0.0.1:9101 just dev-backend
```
