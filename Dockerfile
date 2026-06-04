FROM rust:1.89-bookworm AS builder

WORKDIR /app

# Install dependencies yang sering dibutuhkan Rust
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest dulu agar cache dependency lebih optimal
COPY Cargo.toml Cargo.lock ./

# Dummy source untuk build dependency
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release

# Copy source asli
RUN rm -rf src
COPY src ./src

# Build final
RUN cargo build --release

# ==========================
# Runtime image
# ==========================
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/parpress-app .

EXPOSE 7860

CMD ["./parpress-app"]