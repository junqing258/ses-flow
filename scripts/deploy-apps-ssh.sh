#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_TARGET="root@192.168.110.45"
DEPLOY_SSH_TARGET="${DEPLOY_SSH_TARGET:-$DEFAULT_TARGET}"
DEPLOY_REMOTE_DIR="${DEPLOY_REMOTE_DIR:-/opt/ses-flow}"
DEPLOY_ENV_FILE="${DEPLOY_ENV_FILE:-$ROOT_DIR/.env}"
DEPLOY_COMPOSE_FILE="${DEPLOY_COMPOSE_FILE:-$ROOT_DIR/scripts/docker-compose.remote.yml}"
DEPLOY_IMAGE_REPO="${DEPLOY_IMAGE_REPO:-ses-flow/backend}"
DEPLOY_IMAGE_TAG="${DEPLOY_IMAGE_TAG:-$(git -C "$ROOT_DIR" rev-parse --short HEAD 2>/dev/null || date +%Y%m%d%H%M%S)}"
DEPLOY_VITE_RUNNER_BASE_URL="${DEPLOY_VITE_RUNNER_BASE_URL:-/runner-api}"
DEPLOY_IMAGE_REF="${DEPLOY_IMAGE_REPO}:${DEPLOY_IMAGE_TAG}"
DEPLOY_PLATFORM="${DEPLOY_PLATFORM:-}"
DEPLOY_DEBUG="${DEPLOY_DEBUG:-0}"

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  cat <<EOF
用法：scripts/deploy-runner-ssh.sh

在本地构建 backend Docker 镜像，通过 SSH 传输到远端主机，
上传部署文件，并重启远端容器。

首次使用前先执行（需要本地公钥）： ssh-copy-id ${DEFAULT_TARGET}

可覆盖环境变量：
  DEPLOY_SSH_TARGET            远端 SSH 目标。默认：${DEFAULT_TARGET}
  DEPLOY_REMOTE_DIR            远端工作目录。默认：/opt/ses-flow
  DEPLOY_ENV_FILE              本地待上传的环境变量文件。默认：.env
  DEPLOY_COMPOSE_FILE          远端 compose 模板。默认：scripts/docker-compose.remote.yml
  DEPLOY_IMAGE_REPO            Docker 镜像仓库名。默认：ses-flow/backend
  DEPLOY_IMAGE_TAG             Docker 镜像标签。默认：当前 git 短 SHA
  DEPLOY_VITE_RUNNER_BASE_URL  前端构建参数。默认：/runner-api
  DEPLOY_PLATFORM              目标镜像平台。为空时自动从远端主机探测
  DEPLOY_DEBUG                 输出调试信息。1 表示开启

示例：
  DEPLOY_SSH_TARGET=root@192.168.110.45 scripts/deploy-runner-ssh.sh
EOF
  exit 0
fi

if [[ "$DEPLOY_DEBUG" == "1" ]]; then
  set -x
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

log_step() {
  printf '%s\n' "==> $1"
}

debug_log() {
  if [[ "$DEPLOY_DEBUG" == "1" ]]; then
    printf '%s\n' "[debug] $1"
  fi
}

quote_for_remote_sh() {
  printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"
}

remote_sh() {
  local cmd="$1"
  debug_log "远端执行：$cmd"
  ssh "$DEPLOY_SSH_TARGET" "sh -lc $(quote_for_remote_sh "$cmd")"
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

grep -v '^BACKEND_IMAGE=' "$DEPLOY_ENV_FILE" > "$tmp_env_file" || true
printf '\nBACKEND_IMAGE=%s\n' "$DEPLOY_IMAGE_REF" >> "$tmp_env_file"
debug_log "使用环境变量文件：$DEPLOY_ENV_FILE"
debug_log "临时环境变量文件：$tmp_env_file"
debug_log "远端部署目录：$DEPLOY_REMOTE_DIR"
debug_log "镜像引用：$DEPLOY_IMAGE_REF"

log_step "检查远端 Docker Compose 支持：$DEPLOY_SSH_TARGET"
remote_sh "docker compose version >/dev/null"
remote_sh "docker image load --help >/dev/null"

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
  remote_arch="$(remote_sh "uname -m")"
  DEPLOY_PLATFORM="$(detect_platform "$remote_arch")"
  if [[ -z "$DEPLOY_PLATFORM" ]]; then
    echo "暂不支持的远端架构：$remote_arch" >&2
    echo "请手动设置 DEPLOY_PLATFORM，例如 linux/amd64 或 linux/arm64。" >&2
    exit 1
  fi
  log_step "自动识别远端架构：$remote_arch -> $DEPLOY_PLATFORM"
else
  log_step "使用手动指定的平台：$DEPLOY_PLATFORM"
fi

log_step "本地构建镜像：$DEPLOY_IMAGE_REF ($DEPLOY_PLATFORM)"
docker buildx build \
  --load \
  --platform "$DEPLOY_PLATFORM" \
  --file "$ROOT_DIR/apps/backend/Dockerfile" \
  --tag "$DEPLOY_IMAGE_REF" \
  --build-arg "VITE_RUNNER_BASE_URL=$DEPLOY_VITE_RUNNER_BASE_URL" \
  "$ROOT_DIR"

log_step "准备远端目录：$DEPLOY_REMOTE_DIR"
remote_sh "mkdir -p $(quote_for_remote_sh "$DEPLOY_REMOTE_DIR")"

log_step "上传部署文件"
scp "$DEPLOY_COMPOSE_FILE" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/docker-compose.remote.yml"
scp "$tmp_env_file" "$DEPLOY_SSH_TARGET:$DEPLOY_REMOTE_DIR/.env"

log_step "传输镜像到远端主机"
docker save "$DEPLOY_IMAGE_REF" | gzip | remote_sh "gunzip | docker load"

log_step "重启远端服务"
remote_sh "cd $(quote_for_remote_sh "$DEPLOY_REMOTE_DIR") && docker compose -f docker-compose.remote.yml --env-file .env up -d --force-recreate"

log_step "查看远端服务状态"
remote_sh "cd $(quote_for_remote_sh "$DEPLOY_REMOTE_DIR") && docker compose -f docker-compose.remote.yml --env-file .env ps"

printf '%s\n' "部署完成：$DEPLOY_IMAGE_REF -> $DEPLOY_SSH_TARGET"
