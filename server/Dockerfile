# syntax=docker/dockerfile:1
FROM --platform=$BUILDPLATFORM lukemathwalker/cargo-chef:latest-rust-1-bookworm AS chef
WORKDIR /src

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETARCH
RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc-aarch64-linux-gnu libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-gnu > /rust-target ;; \
      arm64) echo aarch64-unknown-linux-gnu > /rust-target ;; \
      *) echo "unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac \
    && rustup target add "$(cat /rust-target)"
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc

COPY --from=planner /src/recipe.json recipe.json
RUN cargo chef cook --release --target "$(cat /rust-target)" -p updater-server --recipe-path recipe.json

COPY . .
RUN cargo build --release --target "$(cat /rust-target)" -p updater-server \
    && cp "target/$(cat /rust-target)/release/updater-server" /updater-server

FROM debian:bookworm-slim
COPY --from=builder /updater-server /usr/local/bin/updater-server
ENV DATA_DIR=/data BIND=0.0.0.0:8080
VOLUME /data
EXPOSE 8080
CMD ["updater-server"]
