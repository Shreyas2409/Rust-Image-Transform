# Build stage
FROM rust:1.82-slim AS builder

WORKDIR /build

# Copy dependency files first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY . .

# Force rebuild of the actual code
RUN touch src/main.rs

# Install build dependencies and build the release binary
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/* && \
    cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y \
    ca-certificates \
    libssl3 \
    curl && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /build/target/release/imagekit /usr/local/bin/imagekit

# Copy frontend assets
COPY --from=builder /build/frontend /app/frontend

WORKDIR /app

# Create cache directory
RUN mkdir -p /app/cache && chmod 777 /app/cache

# Render sets PORT environment variable automatically
ENV PORT=8080
EXPOSE 8080

# Health check endpoint
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:${PORT}/sign?url=https://example.com/test.jpg || exit 1

# Run the binary
CMD ["imagekit"]
