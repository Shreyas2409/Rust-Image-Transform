# ImageKit Load Testing with Goose

This directory contains load testing tools for the ImageKit service using [Goose](https://book.goose.rs/), a Rust-native load testing framework.

## Quick Start

```bash
# Build the load tester
cargo build --release

# Run a simple test (5 users, 30 seconds)
cargo run --release -- \
    --host http://localhost:3000 \
    --users 5 \
    --hatch-rate 1 \
    --run-time 30s
```

## Prerequisites

1. **Running ImageKit server**: The service must be running before load testing
   ```bash
   cd ..
   cargo run --release
   ```

2. **Test images**: Ensure you have a valid image URL for testing
   - Default: Uses `https://picsum.photos/2000/2000` (public test images)
   - Custom: Modify `src/main.rs` to use your own images

## Test Scenarios

### 1. ImageTransformation (Mixed Workload)
Simulates realistic usage with:
- **Sign URL** (3x weight): Lightweight URL signing operations
- **Fetch Image** (10x weight): Image transformation with varied parameters
- **Health Check** (1x weight): Service health monitoring

### 2. CachePerformance
Tests cache effectiveness:
- **Cached Image** (15x weight): Consistent parameters → cache hits
- **Uncached Image** (5x weight): Unique parameters → cache misses

## Usage Examples

### Local Development Testing
```bash
# Light load
cargo run --release -- \
    --host http://localhost:3000 \
    --users 5 \
    --hatch-rate 1 \
    --run-time 30s

# Medium load
cargo run --release -- \
    --host http://localhost:3000 \
    --users 20 \
    --hatch-rate 2 \
    --run-time 120s
```

### Production Load Testing
```bash
# Test deployed service
cargo run --release -- \
    --host https://img.yourdomain.com \
    --users 50 \
    --hatch-rate 5 \
    --run-time 300s \
    --report-file results.html

# Heavy load test (100 concurrent users)
cargo run --release -- \
    --host https://img.yourdomain.com \
    --users 100 \
    --hatch-rate 10 \
    --run-time 600s \
    --report-file heavy-load.html
```

### Advanced Options
```bash
# Custom user count and duration
cargo run --release -- \
    --host https://img.yourdomain.com \
    --users 30 \
    --hatch-rate 3 \
    --run-time 5m

# Only run specific scenario
cargo run --release -- \
    --host http://localhost:3000 \
    --users 10 \
    --hatch-rate 2 \
    --run-time 1m \
    --scenarios CachePerformance

# Verbose output for debugging
RUST_LOG=info cargo run --release -- \
    --host http://localhost:3000 \
    --users 5 \
    --hatch-rate 1 \
    --run-time 30s
```

## Interpreting Results

Goose provides detailed metrics after each test:

```
-------------------------------------------------------------------------------
 Name                     | # reqs | # fails | Avg (ms) | Min | Max   | Median | p95   | p99   | RPS
--------------------------|--------|---------|----------|-----|-------|--------|-------|-------|-----
 GET /sign                | 450    | 0       | 12       | 8   | 45    | 11     | 18    | 32    | 15.0
 GET /img (cached)        | 2250   | 0       | 8        | 5   | 25    | 7      | 12    | 18    | 75.0
 GET /img (uncached)      | 750    | 0       | 145      | 98  | 320   | 132    | 215   | 285   | 25.0
 GET /health              | 150    | 0       | 3        | 2   | 8     | 3      | 4     | 6     | 5.0
-------------------------------------------------------------------------------
```

### Key Metrics

- **# reqs**: Total number of requests made
- **# fails**: Number of failed requests (errors, timeouts)
- **Avg (ms)**: Average response time
- **Min/Max**: Fastest and slowest response times
- **Median**: 50th percentile (half of requests faster, half slower)
- **p95**: 95th percentile (95% of requests faster than this)
- **p99**: 99th percentile (99% of requests faster than this)
- **RPS**: Requests per second (throughput)

### Performance Expectations

Based on single-core performance:

| Operation | Expected RPS | Latency (p95) | Notes |
|-----------|--------------|---------------|-------|
| Sign URL | 100-200 | <20ms | Lightweight HMAC operation |
| Cache hit (edge) | 500-1000+ | <10ms | Served from Cloudflare |
| Cache hit (origin) | 200-500 | <20ms | Served from disk cache |
| Cache miss (WebP) | 20-50 | <200ms | Full transformation |
| Cache miss (AVIF) | 10-25 | <400ms | Slower encoding |

### What to Look For

✅ **Good Signs:**
- Low error rate (< 1%)
- Consistent latency (small gap between avg and p95)
- High RPS for cached requests
- p99 latency < 2x p95 latency

⚠️ **Warning Signs:**
- Error rate > 1%
- Wide gap between median and p95 (inconsistent performance)
- Cache hits taking > 50ms
- p99 latency >> p95 (tail latency issues)

❌ **Critical Issues:**
- Error rate > 5%
- Timeouts or connection failures
- Increasing latency over time (memory leak?)
- RPS decreasing over time (resource exhaustion?)

## Customizing Tests

Edit `src/main.rs` to customize test scenarios:

### Add a New Transaction
```rust
async fn my_custom_transaction(user: &mut GooseUser) -> TransactionResult {
    // Your test logic here
    let _goose = user.get("/my-endpoint").await?;
    Ok(())
}

// Register in main()
scenario!("MyScenario")
    .register_transaction(transaction!(my_custom_transaction).set_weight(5)?)
```

### Adjust Weights
Change the `.set_weight()` values to control request distribution:
```rust
// More sign requests
.register_transaction(transaction!(sign_url).set_weight(10)?)

// Fewer cache misses
.register_transaction(transaction!(uncached_image).set_weight(1)?)
```

### Use Different Images
```rust
// Random image from Lorem Picsum
let url = format!("https://picsum.photos/2000/2000");

// Specific image
let url = "https://yourdomain.com/path/to/image.jpg";

// Your own test images
let images = vec![
    "https://example.com/image1.jpg",
    "https://example.com/image2.jpg",
];
let url = images[rng.gen_range(0..images.len())];
```

## Comparing with Locust

If you prefer Python, you can also use Locust:

```python
# locustfile.py
from locust import HttpUser, task, between
import time

class ImageKitUser(HttpUser):
    wait_time = between(1, 3)
    
    @task(10)
    def fetch_image(self):
        timestamp = int(time.time()) + 3600
        sign_response = self.client.get(
            f"/sign?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t={timestamp}"
        )
        data = sign_response.json()
        self.client.get(data["signed_url"])
    
    @task(1)
    def health_check(self):
        self.client.get("/health")
```

Run with:
```bash
locust -f locustfile.py --host http://localhost:3000
```

**Goose vs Locust:**
- **Goose**: Faster, lower overhead, native Rust, command-line focused
- **Locust**: Web UI, Python familiarity, more visualization options

## Troubleshooting

### "Connection refused" errors
- Ensure the ImageKit server is running
- Check that you're using the correct host and port

### Low RPS (requests per second)
- Increase `--users` to simulate more concurrent users
- Increase `--hatch-rate` to ramp up faster
- Check if your server is CPU-bound (use `htop`)

### High error rate
- Check server logs for errors
- Ensure image URLs are accessible
- Verify signature generation is working
- Check rate limiting settings

### Results not representative
- Run longer tests (>2 minutes) to get stable results
- Warm up the cache before timing critical tests
- Use production-like hardware for accurate metrics
- Test from a separate machine to avoid resource contention

## CI/CD Integration

Add load testing to your CI pipeline:

```yaml
# .github/workflows/loadtest.yml
name: Load Test

on:
  pull_request:
    branches: [ main ]

jobs:
  loadtest:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      
      - name: Start server
        run: |
          cargo build --release
          cargo run --release &
          sleep 10
      
      - name: Run load test
        run: |
          cd loadtest
          cargo run --release -- \
            --host http://localhost:3000 \
            --users 10 \
            --hatch-rate 2 \
            --run-time 60s \
            --report-file results.html
      
      - name: Upload results
        uses: actions/upload-artifact@v2
        with:
          name: loadtest-results
          path: loadtest/results.html
```

## Resources

- [Goose Book](https://book.goose.rs/) - Official documentation
- [Goose Examples](https://github.com/tag1consulting/goose/tree/main/examples)
- [Performance Testing Best Practices](https://docs.locust.io/en/stable/writing-a-locustfile.html#writing-a-locustfile)

## Next Steps

1. Run baseline tests on your local machine
2. Deploy to production and test with Cloudflare caching
3. Compare cache hit vs cache miss performance
4. Identify bottlenecks and optimize
5. Set up continuous load testing in CI/CD
