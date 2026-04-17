FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /workspace/apps/frontend

ENV PNPM_HOME="/pnpm"
ENV PATH="${PNPM_HOME}:${PATH}"

RUN corepack enable

COPY apps/frontend ./
RUN pnpm install --no-frozen-lockfile

ARG VITE_RUNNER_BASE_URL=/runner-api
ENV VITE_RUNNER_BASE_URL=${VITE_RUNNER_BASE_URL}

RUN pnpm build

FROM rust:1.94-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock ./
COPY apps/backend/Cargo.toml apps/backend/Cargo.toml
COPY apps/runner/Cargo.toml apps/runner/Cargo.toml
COPY apps/backend/src apps/backend/src
COPY apps/runner/src apps/runner/src

RUN cargo build --release -p backend

FROM node:22-bookworm-slim AS runtime

WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /workspace/target/release/backend /usr/local/bin/backend
COPY --from=frontend-builder /workspace/apps/frontend/dist /app/views

ENV BACKEND_LOG=info
ENV BACKEND_STATIC_DIR=/app/views

EXPOSE 6302

ENTRYPOINT ["backend"]
CMD ["--host", "0.0.0.0", "--port", "6302"]
