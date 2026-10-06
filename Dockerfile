# syntax=docker/dockerfile:1

FROM rust:1.96.1-slim-bookworm AS builder

ENV RUSTUP_TOOLCHAIN=1.96.1

ARG PROFILE=release
ARG TARGET_DIR=target/release

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev clang libclang-dev

COPY cli cli
COPY sdk sdk
COPY toolkit-rust toolkit-rust

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml
COPY rust-toolchain.toml rust-toolchain.toml

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target <<EOT
    set -e
    cargo build \
    --locked \
    --profile ${PROFILE} \
    --package nexus-cli --bin nexus
    cp -t /app \
        ${TARGET_DIR}/nexus
EOT

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/nexus /

CMD ["./nexus"]

ENV PORT=8080
