# Cloudflare Edge Caching for Rust Image Transformations: A Production Guide

**Author:** Your Name  
**Date:** December 15, 2024  
**Tags:** Rust, Cloudflare, CDN, Image Optimization, Axum, Performance

---

## TL;DR

This guide demonstrates how to implement Cloudflare-compatible edge caching headers for a Rust-based image transformation service. We'll build a drop-in Axum middleware, deploy to Render.io with Cloudflare DNS, and benchmark performance using Goose load testing.

**Key Results:**
- ✅ 1-day edge cache, 1-year browser cache
- ✅ Zero-cost CDN caching with free Cloudflare
- ✅ Drop-in Axum middleware (<100 lines)
- ✅ Load testing framework for single-core performance validation

---

## Background: The Edge Caching Problem

Modern web applications need to serve transformed images quickly and efficiently. Solutions typically fall into three categories:

1. **Expensive SaaS** (Cloudinary, Imgix): $100-1000+/month
2. **Self-hosted without CDN**: High latency, expensive bandwidth
3. **Self-hosted + CDN**: Best of both worlds, but requires proper header configuration

This guide focuses on option #3: **self-hosted Rust service + free Cloudflare CDN**.

## Architecture Overview

```
User Request
    ↓
Cloudflare Edge (Check cache)
    ↓ (Cache miss)
Your Origin Server (Render.io)
    ↓
Transform & Return with Cache Headers
    ↓
Cloudflare Edge (Cache for 1 day)
    ↓
Subsequent requests served from edge
```

## Understanding Cloudflare's Caching Behavior

Cloudflare caches content based on HTTP headers, but has specific nuances:

### What Cloudflare Caches by Default
- Static files (images, CSS, JS)
- Responses with appropriate `Cache-Control` headers

### What Requires Special Configuration
- HTML and JSON (need Cache Rules)
- Dynamic content (our transformed images!)
- Content with query parameters (our transformation parameters)

### Key Headers for Edge Caching

1. **`Cache-Control`**: Controls both browser and CDN caching
   ```
   Cache-Control: public, max-age=31536000, s-maxage=86400, immutable
   ```
   - `public`: Allows CDN caching
   - `max-age`: Browser cache duration (1 year = 31536000s)
   - `s-maxage`: CDN cache duration (1 day = 86400s) - **overrides max-age for CDNs**
   - `immutable`: Content won't change during cache lifetime

2. **`CDN-Cache-Control`** (Cloudflare-specific): Separate CDN control
   ```
   CDN-Cache-Control: max-age=86400
   ```
   - Only affects Cloudflare, not proxied downstream
   - Provides independent control from browser caching

3. **`Vary`**: Tells caches which request headers affect the response
   ```
   Vary: Accept-Encoding
   ```
   - Cloudflare respects `Accept-Encoding` (for compression)
   - Other `Vary` headers require Cloudflare Workers for custom cache keys

4. **`ETag`**: Enables conditional requests
   ```
   ETag: "abc123"
   ```
   - Allows `If-None-Match` revalidation
   - Reduces bandwidth for unchanged content

## Implementation: Cloudflare Caching Middleware

### Step 1: Define Cache Configuration

We'll create a flexible configuration struct that supports multiple use cases:

```rust
// src/cache/cloudflare.rs

#[derive(Clone, Debug)]
pub struct CloudflareCacheConfig {
    pub edge_max_age: u32,              // CDN edge cache duration
    pub browser_max_age: u32,            // Browser cache duration
    pub public: bool,                    // Public vs private caching
    pub stale_if_error: Option<u32>,     // Serve stale on origin error
    pub stale_while_revalidate: Option<u32>, // Serve stale while updating
    pub immutable: bool,                 // Content is immutable
}

impl Default for CloudflareCacheConfig {
    fn default() -> Self {
        Self {
            edge_max_age: 86400,         // 1 day edge cache
            browser_max_age: 31536000,   // 1 year browser cache
            public: true,
            stale_if_error: Some(86400),
            stale_while_revalidate: Some(60),
            immutable: true,
        }
    }
}
```

### Step 2: Build Cache-Control Headers

Transform configuration into proper HTTP headers:

```rust
impl CloudflareCacheConfig {
    pub fn cache_control_value(&self) -> String {
        if self.edge_max_age == 0 {
            return "no-store, no-cache, must-revalidate".to_string();
        }
        
        let mut parts = Vec::new();
        
        if self.public {
            parts.push("public".to_string());
        }
        
        parts.push(format!("max-age={}", self.browser_max_age));
        parts.push(format!("s-maxage={}", self.edge_max_age));
        
        if self.immutable {
            parts.push("immutable".to_string());
        }
        
        if let Some(seconds) = self.stale_if_error {
            parts.push(format!("stale-if-error={}", seconds));
        }
        
        if let Some(seconds) = self.stale_while_revalidate {
            parts.push(format!("stale-while-revalidate={}", seconds));
        }
        
        parts.join(", ")
    }
    
    pub fn cdn_cache_control_value(&self) -> String {
        format!("max-age={}", self.edge_max_age)
    }
}
```

### Step 3: Create Axum Middleware

Drop-in middleware that works with any Axum router:

```rust
use axum::{
    http::{header, HeaderValue, Request, Response},
    middleware::Next,
    body::Body,
};

pub async fn cloudflare_cache_middleware<B>(
    req: Request<B>,
    next: Next<B>,
) -> Response<Body> {
    let mut response = next.run(req).await;
    
    // Only apply caching headers to successful responses
    if response.status().is_success() {
        let config = CloudflareCacheConfig::for_images();
        
        // Main Cache-Control header
        if let Ok(value) = HeaderValue::from_str(&config.cache_control_value()) {
            response.headers_mut().insert(header::CACHE_CONTROL, value);
        }
        
        // Cloudflare-specific header
        if let Ok(value) = HeaderValue::from_str(&config.cdn_cache_control_value()) {
            response.headers_mut().insert(
                header::HeaderName::from_static("cdn-cache-control"),
                value,
            );
        }
        
        // Vary header (Cloudflare respects this for compression)
        if let Ok(value) = HeaderValue::from_str("Accept-Encoding") {
            response.headers_mut().insert(header::VARY, value);
        }
    }
    
    response
}
```

### Step 4: Integrate with Axum Router

Add middleware to your transformation endpoints:

```rust
// src/lib.rs

pub fn router(config: ImageKitConfig) -> Router {
    use crate::cache::cloudflare_cache_middleware;
    use axum::middleware;
    
    let state = Arc::new(config);
    
    let transform_routes = Router::new()
        .route("/img", get(handler).with_state(state.clone()))
        .route("/upload", post(upload_handler).with_state(state.clone()))
        // Add Cloudflare caching middleware
        .layer(middleware::from_fn(cloudflare_cache_middleware));
    
    Router::new()
        .merge(transform_routes)
        // ... other routes
}
```

## Deployment: Render.io + Cloudflare

### Step 1: Deploy to Render.io

1. **Create `render.yaml`:**
```yaml
services:
  - type: web
    name: imagekit
    env: docker
    plan: starter  # Free tier or upgrade for production
    dockerfilePath: ./Dockerfile
    envVars:
      - key: IMAGEKIT_SECRET
        generateValue: true
      - key: RUST_LOG
        value: info
```

2. **Connect GitHub repo** to Render and deploy

3. **Note your Render URL**: `https://your-app.onrender.com`

### Step 2: Configure Cloudflare DNS

1. **Add your domain** to Cloudflare (free plan works!)

2. **Create CNAME record:**
   ```
   Type: CNAME
   Name: img (or @ for root domain)
   Target: your-app.onrender.com
   Proxy status: Proxied (orange cloud)
   ```

3. **Enable "Cache Everything" Page Rule** (optional but recommended):
   ```
   URL: img.yourdomain.com/img*
   Settings: Cache Level = Cache Everything
   ```

### Step 3: Verify Caching

Test with curl to check headers:

```bash
# First request (cache miss)
curl -I https://img.yourdomain.com/img?url=...&sig=...

# Look for:
# cf-cache-status: MISS
# cache-control: public, max-age=31536000, s-maxage=86400, immutable
# cdn-cache-control: max-age=86400

# Second request (cache hit)
curl -I https://img.yourdomain.com/img?url=...&sig=...

# Look for:
# cf-cache-status: HIT
# age: <seconds since cached>
```

## Load Testing with Goose

To understand single-core performance, we'll use **Goose** - a Rust-native load testing framework.

### Why Goose?

- **Rust-native**: No Python/Java dependencies
- **Accurate**: Low overhead, precise measurements
- **Flexible**: Programmable test scenarios
- **Fast**: Efficiently simulates thousands of users

### Setup

Create `loadtest/Cargo.toml`:

```toml
[package]
name = "imagekit-loadtest"
version = "0.1.0"
edition = "2021"

[dependencies]
goose = "0.17"
tokio = { version = "1", features = ["full"] }
serde_json = "1.0"
rand = "0.8"
chrono = "0.4"
```

### Test Scenarios

Create `loadtest/src/main.rs` with multiple scenarios:

1. **ImageTransformation**: Mixed workload
   - Sign URLs (lightweight)
   - Fetch transformed images (heavy)
   - Health checks (monitoring)

2. **CachePerformance**: Cache effectiveness
   - Cached images (consistent parameters → cache hits)
   - Uncached images (unique parameters → cache misses)

```rust
use goose::prelude::*;

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("ImageTransformation")
                .register_transaction(transaction!(sign_url).set_weight(3)?)
                .register_transaction(transaction!(fetch_image).set_weight(10)?)
                .register_transaction(transaction!(health_check).set_weight(1)?)
        )
        .register_scenario(
            scenario!("CachePerformance")
                .register_transaction(transaction!(cached_image).set_weight(15)?)
                .register_transaction(transaction!(uncached_image).set_weight(5)?)
        )
        .execute()
        .await?;

    Ok(())
}
```

### Running Load Tests

```bash
cd loadtest

# Light load (5 users, 30 seconds)
cargo run --release -- \
    --host http://localhost:3000 \
    --users 5 \
    --hatch-rate 1 \
    --run-time 30s

# Medium load (20 users, 2 minutes)
cargo run --release -- \
    --host http://localhost:3000 \
    --users 20 \
    --hatch-rate 2 \
    --run-time 120s

# Heavy load (100 users, 5 minutes)
cargo run --release -- \
    --host https://img.yourdomain.com \
    --users 100 \
    --hatch-rate 10 \
    --run-time 300s
```

### Interpreting Results

Goose provides detailed metrics:

```
 Name                    | # reqs | # fails | Avg (ms) | Min | Max   | Median | p95   | p99   | RPS
-------------------------|--------|---------|----------|-----|-------|--------|-------|-------|-----
 GET /sign               | 450    | 0       | 12       | 8   | 45    | 11     | 18    | 32    | 15.0
 GET /img (cached)       | 2250   | 0       | 8        | 5   | 25    | 7      | 12    | 18    | 75.0
 GET /img (uncached)     | 750    | 0       | 145      | 98  | 320   | 132    | 215   | 285   | 25.0
 GET /health             | 150    | 0       | 3        | 2   | 8     | 3      | 4     | 6     | 5.0
```

**Key Metrics:**
- **RPS (Requests Per Second)**: Throughput capability
- **Avg/Median**: Typical performance
- **p95/p99**: Tail latency (worst-case performance)
- **# fails**: Error rate

### Expected Performance (Single Core)

Based on workload type:

| Scenario | Expected RPS | Latency (p95) | Notes |
|----------|--------------|---------------|-------|
| Cache hits (edge) | 500-1000+ | <10ms | Served from Cloudflare |
| Cache hits (origin) | 200-500 | <20ms | Served from disk cache |
| Cache miss (WebP) | 20-50 | <200ms | Full transformation |
| Cache miss (AVIF) | 10-25 | <400ms | Slower encoding |

## Production Considerations

### 1. Cache Key Strategy

Our cache keys include transformation parameters:
```rust
fn cache_key(params: &BTreeMap<String, String>) -> String {
    let canonical = params.iter()
        .filter(|(k, _)| k != "sig")
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    
    format!("{:x}", md5::compute(canonical))
}
```

**Why this works:**
- Different transformations = different cache keys
- Same transformation = cache hit
- Query parameters are included in Cloudflare cache key

### 2. Cache Invalidation

When you need to purge cached content:

```bash
# Purge specific file via Cloudflare API
curl -X POST "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache" \
     -H "Authorization: Bearer {api_token}" \
     -H "Content-Type: application/json" \
     --data '{"files":["https://img.yourdomain.com/img?url=...&sig=..."]}'

# Purge everything (use sparingly!)
curl -X POST "https://api.cloudflare.com/client/v4/zones/{zone_id}/purge_cache" \
     -H "Authorization: Bearer {api_token}" \
     -H "Content-Type: application/json" \
     --data '{"purge_everything":true}'
```

### 3. Monitoring

Track cache effectiveness:

```rust
// Add to metrics endpoint
#[derive(Serialize)]
struct CacheMetrics {
    edge_hit_rate: f64,    // Cloudflare CDN hits
    origin_hit_rate: f64,  // Origin cache hits
    transform_rate: f64,   // Actual transformations
}
```

Monitor via Cloudflare Analytics:
- **Cache hit ratio**: Aim for >80%
- **Bandwidth savings**: Should increase over time
- **Origin requests**: Should decrease as cache warms

### 4. Cost Optimization

**Cloudflare Free Tier:**
- Unlimited bandwidth (for cached content)
- 100,000 requests/day
- Perfect for small-medium sites

**Render.io Pricing:**
- Free tier: 750 hours/month (enough for 1 instance)
- Starter: $7/month (always-on, faster builds)
- Pro: $25/month (autoscaling, better performance)

**Expected costs for 1M image views/month:**
- Without CDN: $50-200 (bandwidth + compute)
- With Cloudflare CDN: $7-25 (mostly cached, minimal origin hits)

## Debugging and Troubleshooting

### Cache Not Working

**Check response headers:**
```bash
curl -I https://img.yourdomain.com/img?...
```

Look for:
- ✅ `cf-cache-status: HIT/MISS` (Cloudflare is caching)
- ✅ `cache-control` includes `public` and `s-maxage`
- ❌ `cf-cache-status: DYNAMIC` (not cached - fix headers)
- ❌ `cache-control: private` or `no-store` (prevents caching)

### High Origin Traffic

If Cloudflare isn't caching:

1. **Enable "Cache Everything" Page Rule**
2. **Check query string handling**:
   - Cloudflare: Dashboard → Caching → Configuration
   - Set to "Standard" (includes query strings in cache key)
3. **Verify DNS proxying**: Orange cloud must be enabled

### Slow Transformations

If cache misses are too slow:

1. **Profile transformation pipeline:**
   ```rust
   let start = std::time::Instant::now();
   let img = decode_image(&bytes)?;
   tracing::info!("Decode: {:?}", start.elapsed());
   
   let start = std::time::Instant::now();
   let resized = resize_image(img, w, h)?;
   tracing::info!("Resize: {:?}", start.elapsed());
   
   let start = std::time::Instant::now();
   let encoded = encode_image(&resized, format, quality)?;
   tracing::info!("Encode: {:?}", start.elapsed());
   ```

2. **Optimize based on findings:**
   - **Slow decode**: Pre-validate image formats
   - **Slow resize**: Use faster algorithm (bilinear vs. lanczos)
   - **Slow encode**: Lower quality, use faster formats (WebP > AVIF)

## Alternative: Locust (Python-based)

If you prefer Python, **Locust** is an alternative to Goose:

```python
# locustfile.py
from locust import HttpUser, task, between

class ImageKitUser(HttpUser):
    wait_time = between(1, 3)
    
    @task(10)
    def fetch_cached_image(self):
        # Sign URL
        sign_response = self.client.get(
            "/sign?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t=..."
        )
        data = sign_response.json()
        
        # Fetch image
        self.client.get(data["signed_url"])
    
    @task(1)
    def health_check(self):
        self.client.get("/health")
```

Run with:
```bash
locust -f locustfile.py --host http://localhost:3000
```

I personally prefer **Goose for Rust projects** due to:
- Type safety (no runtime errors in test scenarios)
- Better integration (same dependencies as main app)
- Lower overhead (more accurate performance measurement)

## Conclusion

We've built a production-ready Cloudflare edge caching solution:

✅ **Drop-in Axum middleware** (<100 lines)  
✅ **Proper cache headers** (1-day edge, 1-year browser)  
✅ **Free CDN** with Cloudflare  
✅ **Load testing** with Goose  
✅ **Deployable** to Render.io in minutes  

### Next Steps

1. **Deploy your service** to Render.io
2. **Configure Cloudflare DNS** with orange cloud proxying
3. **Run load tests** to baseline performance
4. **Monitor cache hit rates** via Cloudflare Analytics
5. **Optimize** based on real-world traffic patterns

### Further Reading

- [Cloudflare Cache documentation](https://developers.cloudflare.com/cache/)
- [Goose load testing book](https://book.goose.rs/)
- [Axum middleware guide](https://docs.rs/axum/latest/axum/middleware/index.html)
- [HTTP caching best practices](https://www.mnot.net/cache_docs/)

---

**Questions or feedback?** Open an issue on GitHub or reach out on Twitter!

**Want to see the full code?** Check out the [ImageKit repository](https://github.com/yourusername/imagekit).
