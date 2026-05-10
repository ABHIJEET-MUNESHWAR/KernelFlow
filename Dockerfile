# syntax=docker/dockerfile:1.7

# ---------- builder ----------
# Pinned to a Rust new enough for libp2p 0.54 (>= 1.83) and wasmtime 23.
FROM rust:1.83-slim-bookworm AS builder
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev clang cmake build-essential ca-certificates git \
    libclang-dev protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p kernelflow-node && \
    cp target/release/kernelflow /usr/local/bin/kernelflow

# ---------- runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/bin/kernelflow /usr/local/bin/kernelflow
EXPOSE 8080 9090
ENV RUST_LOG=info
ENTRYPOINT ["/usr/local/bin/kernelflow"]

