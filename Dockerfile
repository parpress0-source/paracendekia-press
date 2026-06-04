# ==========================
# 1. Builder Stage
# ==========================
FROM rust:1.89-bookworm AS builder

WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy manifest untuk caching
COPY Cargo.toml Cargo.lock ./

# [PERBAIKAN CRITICAL] Jalankan dummy build dengan SQLX_OFFLINE agar tidak nyangkut mencari DB
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    SQLX_OFFLINE=true cargo build --release && \
    rm -rf src

# Copy source asli aplikasi
COPY src ./src
# Copy juga folder migrations jika Actix Web Anda menjalankan migrasi SQLx otomatis saat startup
# COPY migrations ./migrations 

# Build final aplikasi asli
RUN cargo build --release

# ==========================
# 2. Runtime Stage
# ==========================
FROM debian:bookworm-slim

# Install ca-certificates (wajib untuk reqwest/Cloudinary HTTPS)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary dari builder
COPY --from=builder /app/target/release/parpress-app .

# [OPTIMASI] Copy folder migrations jika aplikasi Anda butuh menjalankan `sqlx::migrate!()` saat dinyalakan
# COPY --from=builder /app/migrations ./migrations

# Hugging Face Spaces mewajibkan PORT 7860
EXPOSE 7860

# Pastikan environment variable PORT dibaca sebagai 7860 di dalam container
ENV PORT=7860

CMD ["./parpress-app"]