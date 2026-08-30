FROM rust:1.92-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock README.md ./
COPY src ./src
COPY mcp-server/Cargo.toml ./mcp-server/Cargo.toml
COPY mcp-server/src ./mcp-server/src
RUN cargo build --locked --release -p sheetbrief-mcp

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sheetbrief-mcp /usr/local/bin/sheetbrief-mcp

ENV RUST_LOG=sheetbrief_mcp=info \
    SHEETBRIEF_BIND=0.0.0.0:10000
EXPOSE 10000
USER 65532:65532

ENTRYPOINT ["/usr/local/bin/sheetbrief-mcp"]
