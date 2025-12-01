# ImageKit Deployment on Render

**Platform:** Render.com  
**Last Updated:** November 2025  
**Difficulty:** Easy  
**Estimated Time:** 15 minutes

---

## Table of Contents

1. [Overview](#overview)
2. [Prerequisites](#prerequisites)
3. [Quick Start](#quick-start)
4. [Configuration](#configuration)
5. [Custom Domains](#custom-domains)
6. [Monitoring](#monitoring)
7. [Troubleshooting](#troubleshooting)
8. [Cost Estimation](#cost-estimation)

---

## Overview

**Why Render?**
- ✅ **Zero configuration Docker support** - automatically detects Dockerfile
- ✅ **Free SSL/TLS certificates** - HTTPS out of the box
- ✅ **Automatic deployments** - from GitHub/GitLab
- ✅ **Easy scaling** - simple instance type upgrades
- ✅ **Persistent disk** - for your image cache
- ✅ **Environment variables** - secure secret management

**What Render Provides:**
- Fully managed infrastructure
- Automatic HTTPS
- Built-in load balancing
- Health checks
- Rolling deploys with zero downtime

---

## Prerequisites

### 1. Create a Render Account

Visit [render.com](https://render.com) and sign up for a free account.

### 2. Push Your Code to Git

Render deploys from Git repositories (GitHub, GitLab, or Bitbucket).

```bash
# If not already in git
cd /Users/shreyashosagurgachndrashekhar/Downloads/imagekit/hello_world
git init
git add .
git commit -m "Initial commit"

# Create a repo on GitHub, then push
git remote add origin https://github.com/YOUR_USERNAME/imagekit.git
git push -u origin main
```

### 3. Create a Dockerfile

Render uses Docker, so you need a `Dockerfile`. Create one in your project root:

**File:** `Dockerfile`

```dockerfile
# Build stage
FROM rust:1.75-slim as builder

WORKDIR /build

# Copy dependency files first for layer caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy actual source code
COPY . .

# Force rebuild of the actual code
RUN touch src/main.rs

# Build the release binary
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

# Render sets PORT environment variable
ENV PORT=8080
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
  CMD curl -f http://localhost:8080/sign?url=https://example.com/test.jpg || exit 1

CMD ["imagekit"]
```

### 4. Update src/main.rs to Use PORT Environment Variable

Render provides a `PORT` environment variable. Update your code to use it:

**File:** `src/main.rs` (or wherever you bind the server)

```rust
// Read port from environment, default to 8080
let port = std::env::var("PORT")
    .unwrap_or_else(|_| "8080".to_string())
    .parse::<u16>()
    .unwrap_or(8080);

let addr = SocketAddr::from(([0, 0, 0, 0], port));
println!("Server starting on {}", addr);

// ... bind to addr
```

Commit these changes:

```bash
git add Dockerfile src/main.rs
git commit -m "Add Dockerfile and PORT configuration for Render"
git push
```

---

## Quick Start

### Step 1: Create a New Web Service

1. **Log in to Render Dashboard**: [dashboard.render.com](https://dashboard.render.com)
2. **Click "New +"** in the top right
3. **Select "Web Service"**

### Step 2: Connect Your Repository

1. **Connect your GitHub/GitLab account** if you haven't already
2. **Select your ImageKit repository** from the list
3. Click **"Connect"**

### Step 3: Configure the Service

Fill in the deployment configuration:

| Field | Value |
|-------|-------|
| **Name** | `imagekit` (or your preferred name) |
| **Region** | Choose closest to your users (e.g., `Oregon (US West)`) |
| **Branch** | `main` (or your default branch) |
| **Root Directory** | Leave empty (or `.` if needed) |
| **Runtime** | `Docker` |
| **Instance Type** | `Starter` (free) or `Standard` ($7/month) |

### Step 4: Add Environment Variables

Click **"Advanced"** and add these environment variables:

| Key | Value | Notes |
|-----|-------|-------|
| `IMAGEKIT_SECRET` | `<your-secret>` | Generate with `openssl rand -hex 32` |
| `RUST_LOG` | `imagekit=info` | Optional: logging level |

**Generate a strong secret:**
```bash
openssl rand -hex 32
```

### Step 5: Add Persistent Disk (for Cache)

1. Scroll to **"Disks"**
2. Click **"Add Disk"**
3. Configure:
   - **Name**: `imagekit-cache`
   - **Mount Path**: `/app/cache`
   - **Size**: `10 GB` (free tier) or more

### Step 6: Deploy

1. Click **"Create Web Service"**
2. Render will:
   - Clone your repository
   - Build the Docker image
   - Deploy to a live URL
   - Assign a free `.onrender.com` domain

**Initial build takes:** 5-15 minutes (Rust compilation is slow)

### Step 7: Access Your Service

Once deployed, Render provides a URL like:
```
https://imagekit-xyz.onrender.com
```

Test it:
```bash
# Get a signed URL
curl "https://imagekit-xyz.onrender.com/sign?url=https://upload.wikimedia.org/wikipedia/commons/3/3f/JPEG_example_flower.jpg&w=400&f=webp&q=80"

# Visit in browser
open https://imagekit-xyz.onrender.com
```

---

## Configuration

### Environment Variables

Access your service settings and add/modify environment variables:

**Dashboard → Your Service → Environment**

Common variables:

```bash
# Required
IMAGEKIT_SECRET=your-64-character-hex-secret

# Optional
RUST_LOG=imagekit=info,tower_http=info
DISABLE_RATE_LIMIT=0  # Set to 1 to disable
PORT=8080  # Render sets this automatically
```

### Persistent Disk Configuration

**Dashboard → Your Service → Disks**

- **Size**: 10 GB (free) to 500 GB (paid)
- **Mount Path**: `/app/cache`
- **Note**: Data persists across deploys

### Auto-Deploy on Git Push

**Dashboard → Your Service → Settings → Build & Deploy**

- ✅ **Auto-Deploy**: Enabled by default
- Every `git push` to your branch triggers a new deployment
- Render runs health checks before switching traffic

---

## Custom Domains

### Step 1: Add Your Domain

**Dashboard → Your Service → Settings → Custom Domains**

1. Click **"Add Custom Domain"**
2. Enter your domain: `images.yourdomain.com`
3. Click **"Save"**

### Step 2: Configure DNS

Render will show DNS instructions. Add a **CNAME record** to your DNS provider:

| Type | Name | Value |
|------|------|-------|
| CNAME | `images` | `imagekit-xyz.onrender.com` |

**Example (Cloudflare, Namecheap, etc.):**
```
Type: CNAME
Name: images
Value: imagekit-xyz.onrender.com
TTL: Auto
```

### Step 3: Wait for SSL Certificate

Render automatically provisions a **free SSL certificate** via Let's Encrypt.

- ⏱️ Takes 5-10 minutes
- ✅ Auto-renews forever
- 🔒 Your site is now `https://images.yourdomain.com`

---

## Scaling

### Vertical Scaling (More Resources)

**Dashboard → Your Service → Settings → Instance Type**

| Instance Type | vCPU | RAM | Cost |
|---------------|------|-----|------|
| **Starter** (Free) | 0.5 | 512 MB | $0 |
| **Standard** | 1 | 1 GB | $7/month |
| **Standard Plus** | 2 | 2 GB | $15/month |
| **Pro** | 4 | 4 GB | $85/month |
| **Pro Plus** | 8 | 8 GB | $170/month |

**Recommendation:**
- **Hobby/Personal**: Starter (free)
- **Small business**: Standard ($7/month)
- **Production**: Standard Plus or Pro

### Horizontal Scaling (Multiple Instances)

**Dashboard → Your Service → Settings → Scaling**

- Set **Number of Instances**: 1 to 10+
- Render provides automatic load balancing
- **Cost**: Instance price × number of instances

**Note:** With multiple instances, consider:
1. **Shared cache**: Use external cache (Redis) instead of disk
2. **Session affinity**: Not needed for stateless image service

---

## Monitoring

### View Logs

**Dashboard → Your Service → Logs**

Live tail of all instances:
```
2025-11-30 17:00:00 INFO imagekit: Server starting on 0.0.0.0:8080
2025-11-30 17:00:05 INFO imagekit: Serving image /img?url=...
```

**Filter logs:**
```bash
# View in your terminal with Render CLI
render logs -t imagekit
```

### Metrics

**Dashboard → Your Service → Metrics**

View:
- **CPU Usage**: Should be < 80% under normal load
- **Memory Usage**: Watch for memory leaks
- **Request Rate**: Requests per second
- **Response Time**: p50, p95, p99 latency

### Alerts

**Dashboard → Your Service → Alerts**

Set up notifications for:
- High CPU usage
- High memory usage
- Failed health checks
- Deploy failures

---

## Health Checks

Render automatically monitors your service health.

**Default health check (from Dockerfile):**
```dockerfile
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
  CMD curl -f http://localhost:8080/sign?url=https://example.com/test.jpg || exit 1
```

**Configure in Dashboard:**
**Settings → Health & Alerts → Health Check Path**

Default: `/` (checks if server responds)

**Recommended:** `/sign?url=https://example.com/test.jpg`

---

## Troubleshooting

### Build Fails

**Error:** *Build failed during `cargo build`*

**Solutions:**
1. Check logs for specific error
2. Ensure `Cargo.toml` and `Cargo.lock` are committed
3. Verify Dockerfile syntax
4. Try building locally: `docker build .`

### Service Crashes on Startup

**Error:** *Service unhealthy after deploy*

**Check:**
1. `IMAGEKIT_SECRET` environment variable is set
2. PORT binding is correct (use `0.0.0.0`, not `127.0.0.1`)
3. View logs for error messages

```bash
# Common issue: missing secret
Error: environment variable not found: IMAGEKIT_SECRET
```

**Fix:** Add `IMAGEKIT_SECRET` in environment variables

### Images Not Transforming

**Error:** *Signed URLs work, but images don't load*

**Possible causes:**
1. **Disk not mounted**: Check `/app/cache` exists
2. **Memory limit**: Starter tier may run out of RAM
3. **Timeout**: Large images take too long (upgrade tier)

**Check logs:**
```
ERROR imagekit: Failed to fetch image: connection timeout
```

### Cache Not Persisting

**Issue:** Cache resets on every deploy

**Solution:**
1. Ensure disk is mounted at `/app/cache`
2. Verify disk is attached in **Dashboard → Disks**
3. Check that cache writes succeed (logs)

### High Latency

**Issue:** Slow response times

**Solutions:**
1. **Upgrade instance type** (more CPU for image processing)
2. **Use CDN** (Cloudflare, Fastly) in front of Render
3. **Enable caching** (check disk is working)
4. **Reduce image sizes** (lower quality, smaller dimensions)

---

## Cost Estimation

### Free Tier (Starter)

**Included:**
- ✅ 750 hours/month (enough for 1 always-on service)
- ✅ 10 GB persistent disk
- ✅ Free SSL
- ✅ 100 GB bandwidth/month

**Limitations:**
- ⚠️ 0.5 vCPU, 512 MB RAM (may be slow for large images)
- ⚠️ Service spins down after 15 min inactivity (30-60s cold start)

**Best for:** Personal projects, demos, light usage

### Paid Tier (Standard - $7/month)

**Included:**
- ✅ Always-on (no spin down)
- ✅ 1 vCPU, 1 GB RAM
- ✅ 10 GB disk (included)
- ✅ Free SSL
- ✅ 100 GB bandwidth

**Best for:** Small business, production apps with light traffic

### Example Production Setup

**Configuration:**
- **Instance**: Standard Plus (2 vCPU, 2 GB RAM) - $15/month
- **Disk**: 50 GB - $5/month (additional 40 GB)
- **Total**: **$20/month**

**Handles:**
- ~10,000 image transforms/day
- ~300,000 requests/month (with caching)
- Decent response times

---

## Render CLI (Optional)

Install the Render CLI for command-line control:

```bash
# Install
brew install render

# Or with npm
npm install -g render

# Login
render login

# Deploy from CLI
render deploy

# View logs
render logs -t imagekit

# SSH into instance (for debugging)
render ssh imagekit
```

---

## Best Practices

### 1. Use Secrets for IMAGEKIT_SECRET

Never commit secrets to Git. Use Render's environment variables.

### 2. Enable Auto-Deploy

Let Render deploy on every `git push` to your main branch.

### 3. Use Health Checks

Configure proper health check endpoints for faster failure detection.

### 4. Monitor Disk Usage

Set up alerts when cache disk is > 80% full.

### 5. Use CDN

Place Cloudflare or another CDN in front for:
- Global edge caching
- DDoS protection
- Additional bandwidth

### 6. Review Logs Regularly

Check for errors, failed requests, and performance issues.

---

## Comparison: Render vs. Other Platforms

| Feature | Render | Heroku | Railway | Fly.io |
|---------|--------|--------|---------|--------|
| **Free Tier** | ✅ 750h/month | ❌ (paid only) | ✅ $5 credit | ✅ Limited |
| **Docker Support** | ✅ Native | ✅ Via buildpack | ✅ Native | ✅ Native |
| **Persistent Disk** | ✅ 10 GB free | ❌ Add-on only | ✅ Limited | ✅ Volumes |
| **Auto HTTPS** | ✅ Free | ✅ Free | ✅ Free | ✅ Free |
| **Easy Setup** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Rust Support** | ✅ Excellent | ✅ Good | ✅ Excellent | ✅ Excellent |

**Verdict:** Render is excellent for Rust apps with its native Docker support and generous free tier.

---

## Next Steps

1. ✅ **Deploy to Render** (follow Quick Start above)
2. ✅ **Test your deployment** (visit your `.onrender.com` URL)
3. ✅ **Add custom domain** (optional)
4. ✅ **Set up monitoring** (view Metrics tab)
5. ✅ **Configure alerts** (get notified of issues)
6. ✅ **Add CDN** (optional, for global performance)

---

## Additional Resources

- **Render Docs**: [render.com/docs](https://render.com/docs)
- **Rust on Render**: [render.com/docs/deploy-rust](https://render.com/docs/deploy-rust)
- **Custom Domains**: [render.com/docs/custom-domains](https://render.com/docs/custom-domains)
- **Environment Variables**: [render.com/docs/environment-variables](https://render.com/docs/environment-variables)

---

## Support

**Render Support:**
- Community Discord: [discord.gg/render](https://discord.gg/render)
- Email: support@render.com
- Docs: render.com/docs

**ImageKit Issues:**
- Check logs first
- Refer to main `DEPLOYMENT.md` guide
- Open GitHub issue on your repository

---

**🎉 You're ready to deploy ImageKit on Render!**

Start with the [Quick Start](#quick-start) section above and you'll be live in 15 minutes.
