# ImageKit Deployment Guide

**Version:** 1.0  
**Last Updated:** November 2025

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Local Development](#local-development)
4. [Production Deployment](#production-deployment)
5. [Configuration](#configuration)
6. [Monitoring](#monitoring)
7. [Troubleshooting](#troubleshooting)
8. [Security Checklist](#security-checklist)

---

## Overview

ImageKit is a Rust-native image transformation service with:
- **HMAC-SHA256 URL signing** for security
- **Disk caching** with ETag support
- **Rate limiting** (10 req/sec per IP, burst 30)
- **Format support**: JPEG, WebP, AVIF
- **Observability**: Structured logging with tracing

---

## Prerequisites

### System Requirements

**Minimum:**
- CPU: 2 cores
- RAM: 2 GB
- Disk: 10 GB (for cache)
- OS: Linux, macOS, or Windows

**Recommended (Production):**
- CPU: 4+ cores
- RAM: 4+ GB
- Disk: 50+ GB SSD (for cache)
- OS: Linux (Ubuntu 22.04 LTS or similar)

### Software Dependencies

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update stable

# System libraries (Ubuntu/Debian)
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev

# System libraries (macOS)
brew install openssl

# Verify installation
rustc --version  # Should be 1.70+
cargo --version
```

---

## Local Development

### 1. Clone and Build

```bash
cd /path/to/imagekit
cargo build --release
```

### 2. Run Locally

```bash
# Set a strong secret
export IMAGEKIT_SECRET="your-local-dev-secret-key"

# Run the server
cargo run --release
```

**Server will start on:** `http://127.0.0.1:8080`

### 3. Test the Installation

**Open browser:** `http://127.0.0.1:8080/`

**Or test via curl:**

```bash
# Get signed URL
curl "http://127.0.0.1:8080/sign?url=https://upload.wikimedia.org/wikipedia/commons/3/3f/JPEG_example_flower.jpg&w=400&f=webp&q=80"

# Returns:
# {
#   "canonical": "f=webp&q=80&url=https://...&w=400",
#   "sig": "abc123...",
#   "signed_url": "/img?f=webp&q=80&url=https://...&w=400&sig=abc123..."
# }

# Fetch transformed image
curl "http://127.0.0.1:8080/img?..." -o output.webp
```

### 4. Run Tests

```bash
# All tests
cargo test

# Specific test suite
cargo test --test integration
cargo test --test signature 
cargo test --test transform

# With output
cargo test -- --nocapture
```

**Expected:** All 15 tests should pass.

---

## Production Deployment

### Option 1: Systemd Service (Linux)

#### 1. Build Release Binary

```bash
cargo build --release
sudo cp target/release/imagekit /usr/local/bin/
sudo chmod +x /usr/local/bin/imagekit
```

#### 2. Create Service User

```bash
sudo useradd --system --shell /bin/false imagekit
sudo mkdir -p /var/lib/imagekit/cache
sudo chown -R imagekit:imagekit /var/lib/imagekit
```

#### 3. Create Systemd Service

**File:** `/etc/systemd/system/imagekit.service`

```ini
[Unit]
Description=ImageKit Image Transformation Service
After=network.target

[Service]
Type=simple
User=imagekit
Group=imagekit
WorkingDirectory=/var/lib/imagekit
Environment="IMAGEKIT_SECRET=CHANGE_THIS_IN_PRODUCTION"
Environment="RUST_LOG=imagekit=info,tower_http=info"
ExecStart=/usr/local/bin/imagekit
Restart=always
RestartSec=5

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/imagekit/cache

[Install]
WantedBy=multi-user.target
```

#### 4. Start Service

```bash
# Load and enable
sudo systemctl daemon-reload
sudo systemctl enable imagekit
sudo systemctl start imagekit

# Check status
sudo systemctl status imagekit

# View logs
sudo journalctl -u imagekit -f
```

### Option 2: Docker Deployment

#### 1. Create Dockerfile

**File:** `Dockerfile`

```dockerfile
# Build stage
FROM rust:1.75-slim as builder
WORKDIR /build
COPY . .
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/imagekit /usr/local/bin/
COPY --from=builder /build/frontend /app/frontend
WORKDIR /app
RUN mkdir -p /app/cache && chmod 777 /app/cache
EXPOSE 8080
CMD ["imagekit"]
```

#### 2. Build and Run

```bash
# Build image
docker build -t imagekit:latest .

# Run container
docker run -d \
  --name imagekit \
  -p 8080:8080 \
  -e IMAGEKIT_SECRET="your-production-secret" \
  -e RUST_LOG="imagekit=info" \
  -v imagekit-cache:/app/cache \
  --restart unless-stopped \
  imagekit:latest

# View logs
docker logs -f imagekit
```

#### 3. Docker Compose (Optional)

**File:** `docker-compose.yml`

```yaml
version: '3.8'

services:
  imagekit:
    build: .
    ports:
      - "8080:8080"
    environment:
      IMAGEKIT_SECRET: "${IMAGEKIT_SECRET}"
      RUST_LOG: "imagekit=info,tower_http=info"
    volumes:
      - imagekit-cache:/app/cache
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/sign?url=https://example.com/test.jpg"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  imagekit-cache:
```

```bash
# Start
docker-compose up -d

# Stop
docker-compose down
```

### Option 3: Reverse Proxy with Nginx

ImageKit runs on port 8080 by default. Use Nginx for TLS/SSL and load balancing.

**File:** `/etc/nginx/sites-available/imagekit`

```nginx
upstream imagekit_backend {
    server 127.0.0.1:8080;
    # Add more servers for load balancing
    # server 127.0.0.1:8081;
}

server {
    listen 80;
    server_name images.yourdomain.com;
    
    # Redirect HTTP to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name images.yourdomain.com;

    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/images.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/images.yourdomain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # Proxy settings
    location / {
        proxy_pass http://imagekit_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeouts for large images
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
        
        # Buffering
        proxy_buffering on;
        proxy_buffer_size 4k;
        proxy_buffers 8 4k;
    }

    # Rate limiting (in addition to ImageKit's internal)
    limit_req_zone $binary_remote_addr zone=imagekit:10m rate=20r/s;
    limit_req zone=imagekit burst=50 nodelay;
}
```

**Enable and reload:**

```bash
sudo ln -s /etc/nginx/sites-available/imagekit /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

---

## Configuration

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `IMAGEKIT_SECRET` | **Yes** | `local-dev-secret` | HMAC signing secret (min 32 chars) |
| `RUST_LOG` | No | `info` | Log level: `trace`, `debug`, `info`, `warn`, `error` |
| `DISABLE_RATE_LIMIT` | No | (off) | Set to `1` to disable rate limiting |

### Application Configuration

Edit `src/main.rs` to change defaults:

```rust
let cfg = ImageKitConfig {
    secret: std::env::var("IMAGEKIT_SECRET").unwrap_or_else(|_| "local-dev-secret".into()),
    cache_dir: std::path::PathBuf::from("./cache"),  // Change cache location
    max_input_size: 8 * 1024 * 1024,  // 8 MB max input
    allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
    default_format: Some(ImageFormat::webp),
};
```

### Rate Limiting Configuration

Edit `src/lib.rs`:

```rust
GovernorConfigBuilder::default()
    .per_second(10)     // Change requests per second
    .burst_size(30)     // Change burst allowance
    .finish()
```

### Cache Management

**Manual cleanup:**

```bash
# Remove old cache files (older than 7 days)
find ./cache -type f -mtime +7 -delete

# Check cache size
du -sh ./cache
```

**Automatic cleanup (cron):**

```bash
# Add to crontab
crontab -e

# Run daily at 3 AM
0 3 * * * find /var/lib/imagekit/cache -type f -mtime +7 -delete
```

---

## Monitoring

### Logs

**Systemd:**
```bash
sudo journalctl -u imagekit -f --since "10 minutes ago"
```

**Docker:**
```bash
docker logs -f imagekit --tail 100
```

**Log levels:**
```bash
# Debug mode
export RUST_LOG="imagekit=debug,tower_http=debug"

# Production (info only)
export RUST_LOG="imagekit=info,tower_http=warn"
```

### Key Metrics to Monitor

1. **Request Rate**: Requests per second
2. **Cache Hit Rate**: Cache hits vs misses (check logs)
3. **Error Rate**: 4xx/5xx responses
4. **Latency**: Response time (p50, p95, p99)
5. **Disk Usage**: Cache directory size
6. **Memory**: RSS memory usage

### Health Check Endpoint

```bash
# Simple health check (call /sign endpoint)
curl -f "http://localhost:8080/sign?url=https://example.com/test.jpg"
# Exit code 0 = healthy, non-zero = unhealthy
```

### Prometheus Metrics (Future)

Enable `prometheus` feature in `Cargo.toml`:

```toml
[features]
default = ["image-backend"]
prometheus = ["dep:prometheus"]
```

---

## Troubleshooting

### Server Won't Start

**Check secret:**
```bash
# Ensure IMAGEKIT_SECRET is set
echo $IMAGEKIT_SECRET

# Should be at least 16 characters
```

**Check port:**
```bash
# See if port 8080 is in use
lsof -i :8080
netstat -tulpn | grep 8080
```

**Check permissions:**
```bash
# Cache directory must be writable
ls -ld ./cache
# Should show write permissions for the user running imagekit
```

### Images Not Transforming

**Check logs for fetch errors:**
```bash
# Look for "Failed to fetch" messages
tail -f /var/log/imagekit.log | grep "fetch"
```

**Common causes:**
1. **Invalid URL**: Ensure URL is direct image, not HTML page
2. **Size limit**: Image exceeds 8MB (check max_input_size)
3. **Format**: Source format not supported
4. **Network**: Can't reach remote server (firewall, DNS)

**Test fetch manually:**
```bash
curl -I "https://example.com/image.jpg"
# Should return Content-Type: image/*
```

### Signature Errors

**401 Unauthorized:**
- Secret mismatch between sign and img endpoints
- Signature computed incorrectly
- Parameters tampered with

**410 Gone:**
- Timestamp (`t` parameter) is in the past
- Check server clock sync: `timedatectl status`

### Cache Issues

**Cache not working:**
```bash
# Check cache directory exists and is writable
ls -la ./cache
touch ./cache/test && rm ./cache/test
```

**Cache growing too large:**
```bash
# Add cache cleanup cron job (see Configuration > Cache Management)
```

### Performance Problems

**High latency:**
1. **Check network**: Slow remote image downloads
2. **Check CPU**: Image encoding is CPU-intensive
3. **Check disk**: Slow disk I/O for cache

**Monitor with:**
```bash
# CPU usage
top -p $(pgrep imagekit)

# Disk I/O
iotop -p $(pgrep imagekit)

# Network
iftop
```

**Solutions:**
- Scale horizontally (multiple instances + load balancer)
- Use faster disk (SSD) for cache
- Increase CPU cores
- Add Redis cache for distributed deployments

---

## Security Checklist

### Before Production Deployment

- [ ] **Strong secret**: IMAGEKIT_SECRET is 32+ random characters
- [ ] **HTTPS**: Use Nginx/Caddy for TLS encryption
- [ ] **Rate limiting**: Enabled (default: 10 req/sec)
- [ ] **Firewall**: Only port 443 (HTTPS) exposed publicly
- [ ] **User isolation**: Running as non-root user
- [ ] **Disk limits**: Cache directory has size limits
- [ ] **Log monitoring**: Centralized logging configured
- [ ] **Updates**: Keep Rust and dependencies updated
- [ ] **Backups**: Cache can be regenerated, but backup config

### Generating Strong Secret

```bash
# Generate 32-byte random secret
openssl rand -hex 32

# Or
head -c 32 /dev/urandom | base64
```

**Store in environment:**
```bash
# Add to /etc/environment or systemd service file
export IMAGEKIT_SECRET="your-generated-secret-here"
```

### SSRF Protection (Recommended)

Add URL validation in `src/fetch.rs` to block private IPs:

```rust
// Before fetching, validate URL
use url::Url;

fn is_private_ip(host: &str) -> bool {
    // Check for localhost, private ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
    // Return true if private
}

// In fetch_source:
let parsed = Url::parse(url)?;
if let Some(host) = parsed.host_str() {
    if is_private_ip(host) {
        return Err(ImageKitError::InvalidArgument("Private IP not allowed".into()));
    }
}
```

---

## Scaling Strategies

### Horizontal Scaling

**1. Multiple instances behind load balancer:**

```nginx
upstream imagekit_cluster {
    least_conn;  # Use least connections algorithm
    server 127.0.0.1:8080 weight=1;
    server 127.0.0.1:8081 weight=1;
    server 127.0.0.1:8082 weight=1;
}
```

**2. Shared cache (optional):**

Use Redis or NFS-mounted cache directory to share transforms across instances.

### Vertical Scaling

**Increase resources:**
- More CPU cores for parallel encoding
- More RAM for larger images
- Faster SSD for cache I/O

### CDN Integration

**Place CDN in front:**
1. Point CDN to your ImageKit domain
2. Set long cache TTLs (images are immutable with signed URLs)
3. CDN caches transformed images globally

**Example with Cloudflare:**
- Origin: `https://images.yourdomain.com`
- Cache TTL: 1 year
- Cache everything page rule

---

## Backup and Recovery

### What to Backup

1. **Configuration**: `src/main.rs`, environment files
2. **Frontend**: `frontend/` directory
3. **Secrets**: IMAGEKIT_SECRET (securely!)

### What NOT to Backup

- **Cache directory**: Can be regenerated on-demand
- **Binaries**: Can be rebuilt from source

### Recovery Procedure

```bash
# 1. Restore source code
git clone <repository>

# 2. Restore environment
export IMAGEKIT_SECRET="<your-secret>"

# 3. Rebuild
cargo build --release

# 4. Deploy
sudo systemctl restart imagekit

# Cache will rebuild automatically as requests come in
```

---

## Performance Tuning

### Optimize Compilation

```bash
# Enable LTO (Link-Time Optimization) and codegen units
cat >> Cargo.toml <<EOF

[profile.release]
lto = true
codegen-units = 1
EOF

cargo build --release
```

### Increase File Descriptors

```bash
# For systemd service
echo "LimitNOFILE=65536" >> /etc/systemd/system/imagekit.service
sudo systemctl daemon-reload
sudo systemctl restart imagekit
```

### Kernel Tuning (Linux)

```bash
# Increase TCP connection limits
sudo sysctl -w net.core.somaxconn=4096
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=4096

# Make permanent
echo "net.core.somaxconn=4096" | sudo tee -a /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog=4096" | sudo tee -a /etc/sysctl.conf
```

---

## Support and Community

### Getting Help

1. **Check logs**: Most issues are visible in logs
2. **Review tests**: Run `cargo test` to verify functionality
3. **Check this guide**: Search for your issue above
4. **GitHub Issues**: Open an issue on the repository

### Reporting Bugs

Include:
- ImageKit version (Git commit hash)
- Rust version (`rustc --version`)
- OS and version
- Full error logs
- Steps to reproduce

---

## Changelog

### v1.0.0 (Current)
- ✅ HMAC-SHA256 signature verification
- ✅ Rate limiting (10 req/sec)
- ✅ WebP lossy encoding with quality control
- ✅ Structured logging with tracing
- ✅ 15 integration and unit tests
- ✅ Production-ready error handling

---

## Quick Reference

**Start server:**
```bash
IMAGEKIT_SECRET=<secret> cargo run --release
```

**Run tests:**
```bash
cargo test
```

**Build for production:**
```bash
cargo build --release
strip target/release/imagekit  # Reduce binary size
```

**Check health:**
```bash
curl http://localhost:8080/sign?url=https://example.com/test.jpg
```

**View logs:**
```bash
journalctl -u imagekit -f
```

**Restart service:**
```bash
sudo systemctl restart imagekit
```

---

**End of Deployment Guide** 

For questions or issues, check the logs first, then refer to the Troubleshooting section above.
