set shell := ["bash", "-uc"]

default:
  @just --list

dev:
  #!/usr/bin/env bash
  set -euo pipefail
  trap 'kill 0' EXIT INT TERM
  just dev-ai-gateway &
  just dev-backend &
  just dev-frontend &
  wait

dev-plugins:
  #!/usr/bin/env bash
  set -euo pipefail
  trap 'kill 0' EXIT INT TERM
  just dev-plugin-workstation &
  wait

dev-plugin-hello-world:
  cargo watch \
    -w Cargo.toml \
    -w Cargo.lock \
    -w .env \
    -w plugin-apps/hello-world \
    -x "run -p hello-world-plugin -- --host 0.0.0.0 --port ${HELLO_WORLD_PLUGIN_PORT:-9101}"

dev-plugin-workstation:
  cargo watch \
    -w Cargo.toml \
    -w Cargo.lock \
    -w .env \
    -w plugin-apps/workstation \
    -x "run -p workstation-plugin -- --host 0.0.0.0 --port ${WORKSTATION_PLUGIN_PORT:-9102}"

dev-ai-gateway:
  pnpm --filter ai-gateway dev

dev-backend:
  cargo watch \
    -w Cargo.toml \
    -w Cargo.lock \
    -w .env \
    -w apps/backend \
    -w apps/runner \
    -x "run -p backend -- --host 0.0.0.0 --port 6302"

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
  cargo run --release -p hello-world-plugin -- --host 0.0.0.0 --port "${HELLO_WORLD_PLUGIN_PORT:-9101}"

start-plugin-workstation:
  cargo run --release -p workstation-plugin -- --host 0.0.0.0 --port "${WORKSTATION_PLUGIN_PORT:-9102}"

start-ai-gateway:
  pnpm --filter ai-gateway start

start-backend:
  cargo run --release -p backend -- --host 0.0.0.0 --port 6302

start-frontend:
  pnpm --filter frontend preview -- --host 0.0.0.0

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

workstation-plugin-test:
  cargo test -p workstation-plugin
