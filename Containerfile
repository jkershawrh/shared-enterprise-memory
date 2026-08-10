FROM rust:1.87-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release 2>/dev/null || true \
    && rm -rf src

COPY src/ src/
COPY tests/ tests/
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd -r projections && useradd -r -g projections -s /sbin/nologin projections

COPY --from=builder /build/target/release/projections /usr/local/bin/projections

USER projections

EXPOSE 8084 8085

ENTRYPOINT ["projections"]
