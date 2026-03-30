# NestJS + Fastify + LangChain Starter

## Setup

```bash
pnpm install
```

If you want to manage both `server` and `web` via the workspace, run the same command from the repo root.

## Run

```bash
pnpm run start:dev
```

Open:

- `http://localhost:3000/`
- `http://localhost:3000/?name=Codex`

## Investment Advisor Agent

Endpoints:

- `POST /api/advisor` → JSON `{ message, sessionId? }`
- `POST /api/advisor/stream` → SSE stream (`delta`, `done`, `error`)
- `GET /api/advisor/ui` → Interactive UI (Vite build served by backend)

Example:

```bash
curl -X POST http://localhost:3000/api/advisor \\
  -H 'Content-Type: application/json' \\
  -d '{"message":"我想了解黄金与美股的风险差异"}'
```

```bash
curl -N -X POST http://localhost:3000/api/advisor/stream \\
  -H 'Content-Type: application/json' \\
  -d '{"message":"请给我一份ETF对比清单"}'
```

Environment variables:

- `LLM_MODEL` (default: `openai:gpt-4o-mini`)
- `OPENAI_API_KEY` (if using OpenAI models)
- `EXA_API_KEY` (for Exa search)
- `AIGROUP_MARKET_MCP_URL` (Streamable HTTP MCP server URL)
- `AIGROUP_MARKET_MCP_COMMAND` + `AIGROUP_MARKET_MCP_ARGS` (stdio MCP server)
- `AIGROUP_MARKET_MCP_AUTH_TOKEN` (optional bearer token)

## Build

```bash
pnpm run build
pnpm run start
```
