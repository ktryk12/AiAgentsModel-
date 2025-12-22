# ---- build stage ----
FROM rust:1.78-bookworm AS builder

WORKDIR /app

# System deps for typical Rust crates (reqwest/sqlx)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first (cache-friendly)
COPY Cargo.toml Cargo.lock ./
# Copy workspace member manifests
COPY services/orchestrator/Cargo.toml services/orchestrator/Cargo.toml
COPY crates/vdb/Cargo.toml crates/vdb/Cargo.toml
COPY crates/modelops/Cargo.toml crates/modelops/Cargo.toml

# Dummy src to let cargo fetch + compile deps (cache)
RUN mkdir -p services/orchestrator/src && printf "fn main(){}\n" > services/orchestrator/src/main.rs
# Dummy src for crates (needed for flexible workspace build, though simpler usually works)
RUN mkdir -p crates/vdb/src && touch crates/vdb/src/lib.rs
RUN mkdir -p crates/modelops/src && touch crates/modelops/src/lib.rs

# Build deps layer
RUN cargo build -p orchestrator --release

# Now copy real source
COPY services/orchestrator/src services/orchestrator/src
COPY crates crates

# Build real binary (touch main.rs to ensure rebuild)
RUN touch services/orchestrator/src/main.rs
RUN cargo build -p orchestrator --release

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/orchestrator /usr/local/bin/orchestrator

# Create non-root user
RUN useradd -m appuser
USER appuser

ENV RUST_LOG=info

CMD ["orchestrator"]
