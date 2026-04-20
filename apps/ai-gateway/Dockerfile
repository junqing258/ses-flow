FROM node:22-bookworm-slim AS base

WORKDIR /workspace

ENV PNPM_HOME="/pnpm"
ENV PATH="${PNPM_HOME}:${PATH}"
ENV COREPACK_NPM_REGISTRY="https://registry.npmmirror.com"

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && corepack enable

FROM base AS deps

COPY .npmrc package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/ai-gateway/package.json apps/ai-gateway/package.json

RUN pnpm install --filter ai-gateway... --frozen-lockfile --store-dir=/pnpm/store

# 强制下载所有平台的 optionalDependencies 到 store
RUN pnpm add --force --store-dir=/pnpm/store \
  @anthropic-ai/claude-agent-sdk-linux-x64@0.2.114 \
  @anthropic-ai/claude-agent-sdk-linux-x64-musl@0.2.114 \
  @anthropic-ai/claude-code-linux-x64@2.1.114 \
  @anthropic-ai/claude-code-linux-x64-musl@2.1.114 || true

FROM deps AS builder

COPY apps/ai-gateway apps/ai-gateway

RUN pnpm --filter ai-gateway build

FROM base AS runtime

COPY .npmrc package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/ai-gateway/package.json apps/ai-gateway/package.json
COPY --from=deps /pnpm/store /pnpm/store

RUN pnpm install --filter ai-gateway --prod --frozen-lockfile --store-dir=/pnpm/store --offline

COPY . .
COPY --from=builder /workspace/apps/ai-gateway/dist apps/ai-gateway/dist

WORKDIR /workspace/apps/ai-gateway

ENV NODE_ENV=production
ENV AI_GATEWAY_HOST=0.0.0.0
ENV AI_GATEWAY_PORT=6307

EXPOSE 6307

CMD ["node", "dist/index.js"]
