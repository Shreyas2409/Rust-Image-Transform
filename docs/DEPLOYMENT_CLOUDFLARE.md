# Cloudflare + Render.io Deployment Quick Start

## Prerequisites
- [ ] Render.io account (free tier works)
- [ ] Cloudflare account (free tier works)
- [ ] Domain name (or use Cloudflare's free subdomain)
- [ ] GitHub repository with your ImageKit code

## Step 1: Deploy to Render.io

### Via Dashboard
1. Go to [Render Dashboard](https://dashboard.render.com/)
2. Click "New +" → "Web Service"
3. Connect your GitHub repository
4. Configure:
   - **Name**: `imagekit`
   - **Environment**: `Docker`
   - **Plan**: `Starter` ($7/mo) or `Free` (spins down after inactivity)
   - **Docker Command**: (leave default)

### Environment Variables
Add in Render dashboard:
```
IMAGEKIT_SECRET=<generate-random-string>
RUST_LOG=info
DISABLE_RATE_LIMIT=false
```

### Save Your Render URL
After deployment: `https://imagekit-xyz.onrender.com`

## Step 2: Configure Cloudflare DNS

### Add Domain to Cloudflare
1. Go to [Cloudflare Dashboard](https://dash.cloudflare.com/)
2. Click "Add a Site"
3. Enter your domain name
4. Select Free plan
5. Update nameservers at your domain registrar (wait for propagation)

### Create DNS Record
1. Go to DNS → Records
2. Add CNAME record:
   ```
   Type: CNAME
   Name: img (creates img.yourdomain.com)
   Target: imagekit-xyz.onrender.com
   Proxy status: Proxied (🟠 orange cloud)
   TTL: Auto
   ```
3. Click "Save"

### Enable Caching (Optional but Recommended)
1. Go to Rules → Page Rules
2. Create Page Rule:
   ```
   URL: img.yourdomain.com/img*
   Settings:
     - Cache Level: Cache Everything
     - Edge Cache TTL: 1 day
   ```
3. Save and Deploy

## Step 3: Test Your Setup

### Test Origin Server
```bash
# Health check
curl https://imagekit-xyz.onrender.com/health

# Expected:
# {"status":"healthy","version":"0.1.0","service":"imagekit"}
```

### Test Cloudflare Proxying
```bash
# Sign a URL
curl "https://img.yourdomain.com/sign?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t=$(date +%s)"

# Expected:
# {"canonical":"...","sig":"...","signed_url":"/img?..."}
```

### Test Image Transformation
```bash
# First request (cache MISS)
curl -I "https://img.yourdomain.com/img?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t=<timestamp>&sig=<signature>"

# Check headers:
# ✓ cf-cache-status: MISS (first request)
# ✓ cache-control: public, max-age=31536000, s-maxage=86400, immutable
# ✓ cdn-cache-control: max-age=86400
# ✓ content-type: image/webp

# Second request (cache HIT)
curl -I "https://img.yourdomain.com/img?..." # same URL

# Check headers:
# ✓ cf-cache-status: HIT (served from Cloudflare edge!)
# ✓ age: <seconds> (time since cached)
```

## Step 4: Monitor Performance

### Cloudflare Analytics
1. Go to Analytics & Logs → Traffic
2. Monitor:
   - **Requests**: Total requests to your domain
   - **Bandwidth**: Total data transferred
   - **Cached bandwidth**: Data served from cache
   - **Cache hit ratio**: Percentage of cached requests (aim for >80%)

### Application Metrics
Visit your metrics endpoints:
```bash
# Cache statistics
curl https://img.yourdomain.com/stats/cache

# Prometheus metrics
curl https://img.yourdomain.com/metrics
```

## Step 5: Load Testing

### Install Goose Load Tester
```bash
cd loadtest
cargo build --release
```

### Run Local Test (Development)
```bash
# Start your server locally first
cargo run --release

# In another terminal, run load test
cd loadtest
cargo run --release -- \
    --host http://localhost:3000 \
    --users 5 \
    --hatch-rate 1 \
    --run-time 30s
```

### Run Production Test
```bash
cd loadtest
cargo run --release -- \
    --host https://img.yourdomain.com \
    --users 20 \
    --hatch-rate 2 \
    --run-time 120s \
    --report-file results.html
```

### Interpret Results
Look for:
- **RPS**: Requests per second (higher is better)
- **Latency (p95)**: 95th percentile response time
  - Cache hits: <50ms
  - Cache misses: <500ms
- **Error rate**: Should be 0%

## Troubleshooting

### Issue: `cf-cache-status: DYNAMIC`
**Cause**: Cloudflare not caching dynamic content

**Fix**:
1. Add Page Rule with "Cache Everything"
2. Verify query string handling is set to "Standard"
3. Check that response has `Cache-Control: public`

### Issue: `cf-cache-status: BYPASS`
**Cause**: Cloudflare is bypassing cache

**Fix**:
1. Check for cookies in request (use "Bypass Cache on Cookie" rule)
2. Verify `Cache-Control` doesn't have `private` or `no-store`
3. Check Page Rules aren't set to "Bypass Cache"

### Issue: High origin traffic
**Cause**: Low cache hit rate

**Fix**:
1. Check cache hit ratio in Cloudflare Analytics
2. Increase Edge Cache TTL in Page Rules
3. Pre-warm cache with common transformations
4. Verify cache keys are consistent

### Issue: Slow transformations
**Cause**: Cache misses taking too long

**Fix**:
1. Profile your transformation pipeline (see blog post)
2. Reduce image quality for faster encoding
3. Use WebP instead of AVIF (faster encoding)
4. Scale up Render instance (more CPU cores)

### Issue: Render.io free tier sleeping
**Cause**: Free tier spins down after 15 minutes of inactivity

**Fix**:
1. Upgrade to Starter plan ($7/mo) for always-on
2. Use external uptime monitor (UptimeRobot, etc.) to ping every 10 minutes
3. Accept cold starts for low-traffic applications

## Cost Breakdown

### Cloudflare (Free Tier)
- Unlimited bandwidth (for cached content)
- 100,000 requests/day
- Free SSL/TLS
- **Total: $0/month**

### Render.io Options
- **Free Tier**: $0/month
  - 750 hours/month (1 instance)
  - Spins down after 15 min inactivity
  - 0.1 CPU / 512 MB RAM
  
- **Starter**: $7/month
  - Always on
  - 0.5 CPU / 512 MB RAM
  - Better performance
  
- **Pro**: $25/month
  - Autoscaling
  - 1 CPU / 2 GB RAM
  - Priority support

### Estimated Monthly Costs by Traffic

| Monthly Views | Estimated Cost | Plan | Notes |
|---------------|----------------|------|-------|
| <10K | $0 | Free | Low traffic, cold starts OK |
| 10K-100K | $7 | Starter | Always-on, single instance |
| 100K-1M | $7-25 | Starter/Pro | High cache hit ratio helps |
| >1M | $25+ | Pro + scaling | Monitor and optimize |

**With proper Cloudflare caching (>80% hit rate), most traffic never hits your origin server!**

## Next Steps

1. [ ] Monitor cache hit ratio for 1 week
2. [ ] Optimize cache TTLs based on actual traffic
3. [ ] Set up alerts for errors/downtime
4. [ ] Consider adding Cloudflare Workers for advanced caching logic
5. [ ] Implement cache warming for popular transformations

## Resources

- [Render.io Documentation](https://render.com/docs)
- [Cloudflare Cache Documentation](https://developers.cloudflare.com/cache/)
- [Cloudflare Page Rules](https://developers.cloudflare.com/rules/page-rules/)
- [ImageKit Blog Post](./cloudflare-edge-caching.md)

---

**Need help?** Check the [troubleshooting section](#troubleshooting) or open an issue on GitHub.
