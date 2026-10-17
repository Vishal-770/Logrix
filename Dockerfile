# Stage 1: Cargo Chef Base
FROM rust:bookworm AS chef
RUN cargo install cargo-chef --version 0.1.71 --locked
WORKDIR /app

# Stage 2: Prepare Recipe
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Cache Dependencies & Build Binary
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin logrix

# Stage 4: Minimal Production Runtime
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -u 10001 -m -s /bin/bash logrix
USER logrix

COPY --from=builder /app/target/release/logrix /usr/local/bin/logrix
EXPOSE 4000

ENTRYPOINT ["/usr/local/bin/logrix"]
CMD ["all-in-one"]
