# ---- build stage ----
FROM rust:1.83-bookworm AS builder

# Install nightly toolchain to support edition2024 (required by some deps)
RUN rustup toolchain install nightly && rustup default nightly

WORKDIR /app

# System deps for typical Rust crates (reqwest/sqlx)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first (cache-friendly)
COPY Cargo.toml Cargo.lock ./

# Copy ALL workspace member manifests (required for cargo to resolve graph)
COPY services/orchestrator/Cargo.toml services/orchestrator/Cargo.toml
COPY crates/vdb/Cargo.toml crates/vdb/Cargo.toml
COPY crates/modelops/Cargo.toml crates/modelops/Cargo.toml
COPY crates/claims/Cargo.toml crates/claims/Cargo.toml
# Note: examples are in workspace, but we don't strictly need to build them if not depended on. 
# However, cargo workspace might complain if their Cargo.toml is missing.
# Let's verify if we can skip them. If build fails, we add them. 
# Safe bet: create dummy dirs for them if referenced in root Cargo.toml members.
# Root Cargo.toml has: "examples/mvp-demo", "examples/claim-demo"
RUN mkdir -p examples/mvp-demo && touch examples/mvp-demo/Cargo.toml
RUN mkdir -p examples/claim-demo && touch examples/claim-demo/Cargo.toml
# Trick: empty string or dummy content might be needed if cargo reads them.
# Better strategy: modify root Cargo.toml temporarily or just copy the real ones if they exist.
# Let's copy real ones if possible, or just stub. 
# Given we don't want to fail if they exist, let's copy real ones.
COPY examples/mvp-demo/Cargo.toml examples/mvp-demo/Cargo.toml
COPY examples/claim-demo/Cargo.toml examples/claim-demo/Cargo.toml


# Dummy src to let cargo fetch + compile deps (cache) - for ALL members
RUN mkdir -p services/orchestrator/src && printf "fn main(){}\n" > services/orchestrator/src/main.rs
RUN mkdir -p crates/vdb/src && touch crates/vdb/src/lib.rs
RUN mkdir -p crates/modelops/src && touch crates/modelops/src/lib.rs
RUN mkdir -p crates/claims/src && touch crates/claims/src/lib.rs
RUN mkdir -p examples/mvp-demo/src && printf "fn main(){}\n" > examples/mvp-demo/src/main.rs
RUN mkdir -p examples/claim-demo/src && printf "fn main(){}\n" > examples/claim-demo/src/main.rs

# Build deps layer (release)
RUN cargo build -p orchestrator --release

# Now copy real source
COPY services/orchestrator/migrations services/orchestrator/migrations
COPY services services
COPY crates crates
COPY examples examples

# Build real binary (touch all source entrypoints to ensure rebuild due to mtime/cache)
RUN touch services/orchestrator/src/main.rs
RUN touch crates/vdb/src/lib.rs
RUN touch crates/modelops/src/lib.rs
RUN touch crates/claims/src/lib.rs


RUN cargo build -p orchestrator --release

# ---- runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    python3 python3-pip python3-venv \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create venv + install minimal deps
RUN python3 -m venv /opt/venv
ENV PATH="/opt/venv/bin:$PATH"
RUN pip install --no-cache-dir --upgrade pip && \
    pip install --no-cache-dir huggingface_hub requests psycopg2-binary pgvector


COPY --from=builder /app/target/release/orchestrator /usr/local/bin/orchestrator

# Worker scripts
COPY services/orchestrator/workers /app/workers
COPY services/orchestrator/defaults /app/defaults

# Make workers executable
RUN chmod +x /app/workers/*.py

# Create non-root user
RUN useradd -m appuser
USER appuser

ENV RUST_LOG=info

CMD ["orchestrator"]
