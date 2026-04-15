#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_TARGET="root@192.168.110.45"
DEPLOY_SSH_TARGET="${DEPLOY_SSH_TARGET:-$DEFAULT_TARGET}"
DEPLOY_REMOTE_DIR="${DEPLOY_REMOTE_DIR:-/opt/ses-flow}"
DEPLOY_ENV_FILE="${DEPLOY_ENV_FILE:-$ROOT_DIR/.env}"
DEPLOY_COMPOSE_FILE="${DEPLOY_COMPOSE_FILE:-$ROOT_DIR/docker-compose.remote.yml}"
DEPLOY_IMAGE_REPO="${DEPLOY_IMAGE_REPO:-ses-flow/runner}"
DEPLOY_IMAGE_TAG="${DEPLOY_IMAGE_TAG:-$(git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || date +%Y%m%d%H%M%S)}"
DEPLOY_VITE_RUNNER_BASE_URL="${DEPLOY_VITE_RUNNER_BASE_URL:-/runner-api}"
DEPLOY_IMAGE_REF="${DEPLOY_IMAGE_REPO}:${DEPLOY_IMAGE_TAG}"
DEPLOY_PLATFORM="${DEPLOY_PLATFORM:-}"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<EOF
Usage: scripts/deploy-runner-ssh.sh

Build the runner Docker image locally, stream it to a remote host over SSH,
upload the deployment bundle, and restart the remote container.

Environment overrides:
  DEPLOY_SSH_TARGET            Remote SSH target. Default: ${DEFAULT_TARGET}
  DEPLOY_REMOTE_DIR            Remote working directory. Default: /opt/ses-flow
  DEPLOY_ENV_FILE              Local env file to upload. Default: .env
  DEPLOY_COMPOSE_FILE          Remote compose template. Default: docker-compose.remote.yml
  DEPLOY_IMAGE_REPO            Docker image repository. Default: ses-flow/runner
  DEPLOY_IMAGE_TAG             Docker image tag. Default: current git short SHA
  DEPLOY_VITE_RUNNER_BASE_URL  Frontend build arg. Default: /runner-api
  DEPLOY_PLATFORM              Target image platform. Auto-detected from remote host when empty

Example:
  DEPLOY_SSH_TARGET=root@192.168.110.45 scripts/deploy-runner-ssh.sh
EOF
  exit 0
fi

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing required command: $cmd" >&2
    exit 1
  fi
}

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "Required file not found: $path" >&2
    exit 1
  fi
}

require_command docker
require_command ssh
require_command scp
require_command gzip
require_file "$DEPLOY_ENV_FILE"
require_file "$DEPLOY_COMPOSE_FILE"

if ! docker buildx version >/dev/null 2>&1; then
  echo "docker buildx is required for cross-platform builds" >&2
  exit 1
fi

if ! grep -Eq '^DATABASE_URL=.+' "$DEPLOY_ENV_FILE"; then
  echo "DATABASE_URL is missing in $DEPLOY_ENV_FILE" >&2
  exit 1
fi

tmp_env_file="$(mktemp)"
cleanup() {
  rm -f "$tmp_env_file"
}
trap cleanup EXIT

grep -v '^RUNNER_IMAGE=' "$DEPLOY_ENV_FILE" > "$tmp_env_file" || true
printf '\nRUNNER_IMAGE=%s\n' "$DEPLOY_IMAGE_REF" >> "$tmp_env_file"

echo "==> Verifying remote Docker Compose support on $DEPLOY_SSH_TARGET"
ssh "$DEPLOY_SSH_TARGET" "docker compose version >/dev/null"

detect_platform() {
  local remote_arch="$1"
  case "$remote_arch" in
    x86_64|amd64)
      echo "linux/amd64"
      ;;
    aarch64|arm64)
      echo "linux/arm64"
      ;;
    *)
      echo ""
      ;;
  esac
}

if [[ -z "$DEPLOY_PLATFORM" ]]; then
  remote_arch="$(ssh "$DEPLOY_SSH_TARGET" "uname -m")"
  DEPLOY_PLATFORM="$(detect_platform "$remote_arch")"
  if [[ -z "$DEPLOY_PLATFORM" ]]; then
    echo "Unsupported remote architecture: $remote_arch" >&2
    echo "Set DEPLOY_PLATFORM manually, for example linux/amd64 or linux/arm64." >&2
    exit 1
  fi
  echo "==> Auto-detected remote architecture: $remote_arch -> $DEPLOY_PLATFORM"
else
  echo "==> Using overridden target platform: $DEPLOY_PLATFORM"
fi

echo "==> Building local image: $DEPLOY_IMAGE_REF ($DEPLOY_PLATFORM)"
docker buildx build \
  --load \
  --platform "$DEPLOY_PLATFORM" \
  --file "$ROOT_DIR/apps/runner/Dockerfile" \
  --tag "$DEPLOY_IMAGE_REF" \
  --build-arg "VITE_RUNNER_BASE_URL=$DEPLOY_VITE_RUNNER_BASE_URL" \
  "$ROOT_DIR"

echo "==> Preparing remote directory: $DEPLOY_REMOTE_DIR"
ssh "$DEPLOY_SSH_TARGET" "mkdir -p '$DEPLOY_REMOTE_DIR'"

echo "==> Uploading deploy files"
scp "$DEPLOY_COMPOSE_FILE" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/docker-compose.remote.yml"
scp "$tmp_env_file" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/.env"

echo "==> Streaming image to remote host"
docker save "$DEPLOY_IMAGE_REF" | gzip | ssh "$DEPLOY_SSH_TARGET" "gunzip | docker load"

echo "==> Restarting remote service"
ssh "$DEPLOY_SSH_TARGET" "
  cd '$DEPLOY_REMOTE_DIR' && \
  docker compose -f docker-compose.remote.yml --env-file .env up -d --force-recreate
"

echo "==> Remote service status"
ssh "$DEPLOY_SSH_TARGET" "
  cd '$DEPLOY_REMOTE_DIR' && \
  docker compose -f docker-compose.remote.yml --env-file .env ps
"

echo "Deployment completed: $DEPLOY_IMAGE_REF -> $DEPLOY_SSH_TARGET"
