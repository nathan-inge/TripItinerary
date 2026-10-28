# syntax=docker/dockerfile:1
#
# Single-image build: SPA → embedded into the Rust binary → slim runtime.
#   docker build -t tripit .
#   docker build --build-arg VITE_INTAKE_ADDRESS=trips@in.example.com -t tripit .

FROM node:20-bookworm-slim AS frontend
WORKDIR /src/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
# Email intake address shown in the UI. Empty hides email intake.
ARG VITE_INTAKE_ADDRESS=
ENV VITE_INTAKE_ADDRESS=${VITE_INTAKE_ADDRESS}
RUN npm run build

FROM rust:1.94-slim-bookworm AS backend
WORKDIR /src/backend
COPY backend/ ./
COPY --from=frontend /src/frontend/dist /src/frontend/dist
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/backend/target \
    cargo build --release --locked \
    && cp target/release/tripit /usr/local/bin/tripit

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home-dir /data tripit \
    && mkdir -p /data && chown tripit /data
COPY --from=backend /usr/local/bin/tripit /usr/local/bin/tripit
USER tripit
ENV DATABASE_URL=sqlite:///data/db.sqlite?mode=rwc \
    BIND_ADDR=0.0.0.0:5050 \
    RUST_LOG=tripit=info,tower_http=info
VOLUME /data
EXPOSE 5050
ENTRYPOINT ["/usr/local/bin/tripit"]
