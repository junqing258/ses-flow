FROM node:22-bookworm-slim AS deps

WORKDIR /workspace

ENV PNPM_HOME="/pnpm"
ENV PATH="${PNPM_HOME}:${PATH}"

RUN corepack enable

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/ai-gateway/package.json apps/ai-gateway/package.json

RUN pnpm install --filter ai-gateway... --frozen-lockfile

FROM deps AS builder

COPY apps/ai-gateway apps/ai-gateway

RUN pnpm --filter ai-gateway build

FROM node:22-bookworm-slim AS runtime

WORKDIR /workspace

ENV PNPM_HOME="/pnpm"
ENV PATH="${PNPM_HOME}:${PATH}"

RUN corepack enable \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/ai-gateway/package.json apps/ai-gateway/package.json

RUN pnpm install --filter ai-gateway --prod --frozen-lockfile

COPY . .
COPY --from=builder /workspace/apps/ai-gateway/dist apps/ai-gateway/dist

WORKDIR /workspace/apps/ai-gateway

ENV NODE_ENV=production
ENV AI_GATEWAY_HOST=0.0.0.0
ENV AI_GATEWAY_PORT=6307

EXPOSE 6307

CMD ["node", "dist/index.js"]
