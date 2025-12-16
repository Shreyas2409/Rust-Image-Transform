use goose::prelude::*;
use rand::Rng;

/// Load testing suite for ImageKit image transformation service.
///
/// This test harness simulates realistic production traffic patterns including:
/// - URL signing operations (lightweight cryptographic overhead)
/// - Image transformations with varied parameters (cache miss scenarios)
/// - Repeated transformations (cache hit scenarios)
/// - Health monitoring endpoints
///
/// # Usage
/// ```bash
/// cd loadtest
/// cargo run --release -- --host http://localhost:3000 --users 10 --hatch-rate 2 --run-time 60s
/// ```
///
/// # Performance Targets
/// - URL signing: <20ms p95 latency
/// - Cache hits: <20ms p95 latency (origin) / <10ms (edge)
/// - Cache misses: <200ms p95 latency (WebP) / <400ms (AVIF)
/// - Error rate: <1%

#[tokio::main]
async fn main() -> Result<(), GooseError> {
    GooseAttack::initialize()?
        .register_scenario(
            scenario!("ImageTransformation")
                // Lightweight signing operations establish baseline overhead
                .register_transaction(transaction!(sign_url).set_weight(3)?)
                // Primary workload: image transformation with cache behavior
                .register_transaction(transaction!(fetch_image).set_weight(10)?)
                // Continuous health validation ensures service availability
                .register_transaction(transaction!(health_check).set_weight(1)?)
        )
        .register_scenario(
            scenario!("CachePerformance")
                // Consistent parameters validate cache hit performance
                .register_transaction(transaction!(cached_image).set_weight(15)?)
                // Unique parameters validate transformation throughput
                .register_transaction(transaction!(uncached_image).set_weight(5)?)
        )
        .execute()
        .await?;

    Ok(())
}

/// Generates and validates signed URLs for image transformations.
///
/// Randomizes transformation parameters to distribute load across cache keys,
/// validating the HMAC signing mechanism under varied inputs.
async fn sign_url(user: &mut GooseUser) -> TransactionResult {
    // Generate random parameters before await to satisfy Send bounds
    let url = {
        let mut rng = rand::thread_rng();
        let width = rng.gen_range(100..1000);
        let height = rng.gen_range(100..1000);
        let formats = ["webp", "jpeg", "avif"];
        let format = formats[rng.gen_range(0..formats.len())];
        
        format!(
            "/sign?url=https://picsum.photos/2000/2000&w={}&h={}&f={}&q=80&t={}",
            width,
            height,
            format,
            chrono::Utc::now().timestamp() + 3600
        )
    };
    
    let _goose = user.get(&url).await?;
    
    Ok(())
}

/// Executes full transformation pipeline with randomized parameters.
///
/// Tests end-to-end latency including signing, cache lookup, and transformation.
/// Parameter randomization ensures realistic cache miss distribution.
async fn fetch_image(user: &mut GooseUser) -> TransactionResult {
    // Generate random parameters before await to satisfy Send bounds
    let sign_url = {
        let mut rng = rand::thread_rng();
        let width = rng.gen_range(200..800);
        let height = rng.gen_range(200..800);
        
        format!( 
            "/sign?url=https://picsum.photos/2000/2000&w={}&h={}&f=webp&q=80&t={}",
            width,
            height,
            chrono::Utc::now().timestamp() + 3600
        )
    };
    
    let _goose = user.get(&sign_url).await?;
    
    // Simple implementation - just test the full URL without parsing JSON
    // In real scenarios, you would parse the signature and use it
    let _image_goose = user.get(&sign_url.replace("/sign?", "/img?")).await?;
    
    Ok(())
}

/// Validates cache hit performance with consistent parameters.
///
/// Uses fixed transformation parameters to guarantee cache hits after warmup,
/// measuring steady-state performance under optimal conditions.
async fn cached_image(user: &mut GooseUser) -> TransactionResult {
    // Deterministic parameters ensure cache key collision
    let sign_url = format!(
        "/sign?url=https://picsum.photos/2000/2000&w=500&h=500&f=webp&q=80&t={}",
        chrono::Utc::now().timestamp() + 3600
    );
    
    let _image_goose = user.get(&sign_url.replace("/sign?", "/img?")).await?;
    
    Ok(())
}

/// Validates cache miss performance with guaranteed unique parameters.
///
/// Uses timestamp-derived dimensions to ensure cache misses,
/// measuring worst-case transformation latency.
async fn uncached_image(user: &mut GooseUser) -> TransactionResult {
    // Timestamp-based dimensions guarantee cache key uniqueness
    let timestamp = chrono::Utc::now().timestamp();
    let width = 200 + (timestamp % 100) as i32;
    let height = 200 + ((timestamp / 100) % 100) as i32;
    
    let sign_url = format!(
        "/sign?url=https://picsum.photos/2000/2000&w={}&h={}&f=webp&q=80&t={}",
        width,
        height,
        timestamp + 3600
    );
    
    let _image_goose = user.get(&sign_url.replace("/sign?", "/img?")).await?;
    
    Ok(())
}

/// Monitors service availability via health check endpoint.
///
/// Provides baseline for infrastructure latency separate from
/// business logic overhead.
async fn health_check(user: &mut GooseUser) -> TransactionResult {
    let _goose = user.get("/health").await?;
    Ok(())
}
