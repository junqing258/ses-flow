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
用法：scripts/deploy-runner-ssh.sh

在本地构建 runner Docker 镜像，通过 SSH 传输到远端主机，
上传部署文件，并重启远端容器。

可覆盖环境变量：
  DEPLOY_SSH_TARGET            远端 SSH 目标。默认：${DEFAULT_TARGET}
  DEPLOY_REMOTE_DIR            远端工作目录。默认：/opt/ses-flow
  DEPLOY_ENV_FILE              本地待上传的环境变量文件。默认：.env
  DEPLOY_COMPOSE_FILE          远端 compose 模板。默认：docker-compose.remote.yml
  DEPLOY_IMAGE_REPO            Docker 镜像仓库名。默认：ses-flow/runner
  DEPLOY_IMAGE_TAG             Docker 镜像标签。默认：当前 git 短 SHA
  DEPLOY_VITE_RUNNER_BASE_URL  前端构建参数。默认：/runner-api
  DEPLOY_PLATFORM              目标镜像平台。为空时自动从远端主机探测

示例：
  DEPLOY_SSH_TARGET=root@192.168.110.45 scripts/deploy-runner-ssh.sh
EOF
  exit 0
fi

require_command() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "缺少必需命令：$cmd" >&2
    exit 1
  fi
}

require_file() {
  local path="$1"
  if [[ ! -f "$path" ]]; then
    echo "缺少必需文件：$path" >&2
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
  echo "跨平台构建需要 docker buildx" >&2
  exit 1
fi

if ! grep -Eq '^DATABASE_URL=.+' "$DEPLOY_ENV_FILE"; then
  echo "$DEPLOY_ENV_FILE 中缺少 DATABASE_URL" >&2
  exit 1
fi

tmp_env_file="$(mktemp)"
cleanup() {
  rm -f "$tmp_env_file"
}
trap cleanup EXIT

grep -v '^RUNNER_IMAGE=' "$DEPLOY_ENV_FILE" > "$tmp_env_file" || true
printf '\nRUNNER_IMAGE=%s\n' "$DEPLOY_IMAGE_REF" >> "$tmp_env_file"

echo "==> 检查远端 Docker Compose 支持：$DEPLOY_SSH_TARGET"
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
    echo "暂不支持的远端架构：$remote_arch" >&2
    echo "请手动设置 DEPLOY_PLATFORM，例如 linux/amd64 或 linux/arm64。" >&2
    exit 1
  fi
  echo "==> 自动识别远端架构：$remote_arch -> $DEPLOY_PLATFORM"
else
  echo "==> 使用手动指定的平台：$DEPLOY_PLATFORM"
fi

echo "==> 本地构建镜像：$DEPLOY_IMAGE_REF ($DEPLOY_PLATFORM)"
docker buildx build \
  --load \
  --platform "$DEPLOY_PLATFORM" \
  --file "$ROOT_DIR/apps/runner/Dockerfile" \
  --tag "$DEPLOY_IMAGE_REF" \
  --build-arg "VITE_RUNNER_BASE_URL=$DEPLOY_VITE_RUNNER_BASE_URL" \
  "$ROOT_DIR"

echo "==> 准备远端目录：$DEPLOY_REMOTE_DIR"
ssh "$DEPLOY_SSH_TARGET" "mkdir -p '$DEPLOY_REMOTE_DIR'"

echo "==> 上传部署文件"
scp "$DEPLOY_COMPOSE_FILE" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/docker-compose.remote.yml"
scp "$tmp_env_file" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/.env"

echo "==> 传输镜像到远端主机"
docker save "$DEPLOY_IMAGE_REF" | gzip | ssh "$DEPLOY_SSH_TARGET" "gunzip | docker load"

echo "==> 重启远端服务"
ssh "$DEPLOY_SSH_TARGET" "
  cd '$DEPLOY_REMOTE_DIR' && \
  docker compose -f docker-compose.remote.yml --env-file .env up -d --force-recreate
"

echo "==> 查看远端服务状态"
ssh "$DEPLOY_SSH_TARGET" "
  cd '$DEPLOY_REMOTE_DIR' && \
  docker compose -f docker-compose.remote.yml --env-file .env ps
"

echo "部署完成：$DEPLOY_IMAGE_REF -> $DEPLOY_SSH_TARGET"
