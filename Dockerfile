# syntax=docker/dockerfile:1.7

# Build stage
FROM rust:1.90.0-alpine3.20 AS builder

WORKDIR /usr/src/app

# Ensure Rust links dynamically against musl so libpq works
ENV RUSTFLAGS="-C target-feature=-crt-static"

# System dependencies for building Diesel (libpq) and TLS support
RUN apk add --no-cache \
        build-base \
        pkgconf \
        openssl-dev \
        postgresql-dev \
        ca-certificates

# Copy manifests separately for better caching
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Prime the Cargo cache with dependency builds using BuildKit cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/app/target \
    <<'SCRIPT'
set -eux
mkdir -p src
printf 'fn main() {}\n' > src/main.rs
cargo build --release --locked
rm -rf src
SCRIPT

# Copy source tree and assets
COPY src ./src
COPY migrations ./migrations
COPY rustfmt.toml ./

# Build the release binary with cached dependencies and persist it in the layer
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/usr/src/app/target \
    <<'SCRIPT'
set -eux
cargo build --release --locked
install -Dm0755 target/release/tado-timescale /usr/local/bin/tado-timescale
SCRIPT


FROM alpine:3.20 AS runtime

WORKDIR /app

# Runtime dependencies for libpq, TLS, libgcc runtime, plus CA bundle for HTTPS
RUN apk add --no-cache \
        libgcc \
        postgresql-libs \
        openssl \
        ca-certificates

COPY --from=builder /usr/local/bin/tado-timescale /usr/local/bin/tado-timescale

ENTRYPOINT ["/usr/local/bin/tado-timescale"]
