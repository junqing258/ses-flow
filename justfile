set shell := ["bash", "-uc"]

default:
  @just --list

dev:
  #!/usr/bin/env bash
  set -euo pipefail
  trap 'kill 0' EXIT INT TERM
  just dev-plugin-hello-world &
  just dev-ai-gateway &
  just dev-backend &
  just dev-frontend &
  wait

dev-plugin-hello-world:
  cargo run -p hello-world-plugin -- --host 127.0.0.1 --port "${HELLO_WORLD_PLUGIN_PORT:-9101}"

dev-ai-gateway:
  pnpm --filter ai-gateway dev

dev-backend:
  cargo watch \
    -w Cargo.toml \
    -w Cargo.lock \
    -w .env \
    -w apps/backend \
    -w apps/runner \
    -x "run -p backend -- --host 127.0.0.1 --port 6302"

dev-frontend:
  pnpm --filter frontend dev

build:
  cargo build --workspace
  pnpm --filter ai-gateway build
  pnpm --filter frontend build

start:
  #!/usr/bin/env bash
  set -euo pipefail
  trap 'kill 0' EXIT INT TERM
  just start-ai-gateway &
  just start-backend &
  just start-frontend &
  wait

start-plugin-hello-world:
  cargo run --release -p hello-world-plugin -- --host 127.0.0.1 --port "${HELLO_WORLD_PLUGIN_PORT:-9101}"

start-ai-gateway:
  pnpm --filter ai-gateway start

start-backend:
  cargo run --release -p backend -- --host 127.0.0.1 --port 6302

start-frontend:
  pnpm --filter frontend preview -- --host 127.0.0.1

test:
  cargo test --workspace
  pnpm --filter ai-gateway test
  pnpm --filter frontend test

lint:
  pnpm --filter frontend lint

format:
  pnpm --filter frontend format

format-check:
  pnpm --filter frontend format:check

backend-test:
  cargo test -p backend

runner-test:
  cargo test -p runner --lib
