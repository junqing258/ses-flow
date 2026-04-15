FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /workspace

ENV PNPM_HOME="/pnpm"
ENV PATH="${PNPM_HOME}:${PATH}"

RUN corepack enable

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/frontend/package.json apps/frontend/package.json
COPY packages/scheme/package.json packages/scheme/package.json

RUN pnpm install --frozen-lockfile --filter frontend...

COPY apps/frontend apps/frontend

ARG VITE_RUNNER_BASE_URL=/runner-api
ENV VITE_RUNNER_BASE_URL=${VITE_RUNNER_BASE_URL}

WORKDIR /workspace/apps/frontend

RUN pnpm build

FROM rust:1.86-bookworm AS builder

WORKDIR /workspace/apps/runner

COPY apps/runner/Cargo.toml apps/runner/Cargo.lock ./
COPY apps/runner/src ./src

RUN cargo build --release

FROM node:22-bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/apps/runner/target/release/runner /usr/local/bin/runner
COPY --from=frontend-builder /workspace/apps/frontend/dist /app/views

ENV RUNNER_LOG=info
ENV RUNNER_STATIC_DIR=/app/views

EXPOSE 6302

ENTRYPOINT ["runner"]
CMD ["--host", "0.0.0.0", "--port", "6302"]
