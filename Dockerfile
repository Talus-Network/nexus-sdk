# syntax=docker/dockerfile:1

FROM rust:1.86-slim AS builder

ARG PROFILE=release
ARG TARGET_DIR=target/release

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev

COPY cli cli
COPY tools tools
COPY sdk sdk
COPY toolkit-rust toolkit-rust

COPY Cargo.lock Cargo.lock
COPY Cargo.toml Cargo.toml
COPY rust-toolchain.toml rust-toolchain.toml

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/sui/${TARGET_DIR} <<EOT
    cargo build \
    --profile ${PROFILE} \
    --bin nexus-cli \
    --bin math \
    --bin llm-openai-chat-completion
    cp -t /app \
        ${TARGET_DIR}/nexus-cli \
        ${TARGET_DIR}/math \
        ${TARGET_DIR}/llm-openai-chat-completion
EOT

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /app/nexus-cli /
COPY --from=builder /app/llm-openai-chat-completion /tools/
COPY --from=builder /app/math /tools/

CMD ["./nexus-cli"]

ENV PORT=8080
