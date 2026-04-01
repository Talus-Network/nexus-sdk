# syntax=docker/dockerfile:1

FROM rust:1.86-slim AS builder

ARG PROFILE=release
ARG TARGET_DIR=target/release

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev

COPY cli cli
COPY sdk sdk
COPY toolkit-rust toolkit-rust

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml
COPY rust-toolchain.toml rust-toolchain.toml

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/sui/${TARGET_DIR} <<EOT
    cargo build \
    --profile ${PROFILE} \
    --bin nexus-cli
    cp -t /app \
        ${TARGET_DIR}/nexus-cli
EOT

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/nexus-cli /

CMD ["./nexus-cli"]

ENV PORT=8080
