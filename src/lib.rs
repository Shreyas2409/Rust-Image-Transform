use axum::{
    extract::Query,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
    body::Body,
    Json,
};
use axum::extract::Multipart;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;
use hmac::Hmac;
use hmac::Mac;
use sha2::Sha256;
use tower_http::services::ServeDir;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

pub mod config;
pub mod signature;
pub mod cache;
pub mod transform;
pub mod fetch;
#[cfg(feature = "prometheus")]
pub mod metrics;

use crate::cache::{Cache, DiskCache};
use crate::config::{ImageFormat, ImageKitConfig, DEFAULT_QUALITY, DEFAULT_CACHE_CONTROL, NO_CACHE_CONTROL};
use crate::fetch::fetch_source;
use crate::signature::verify_signature;
use crate::transform::{encode_image, resize_image, decode_image};

#[derive(Error, Debug)]
pub enum ImageKitError {
    #[error("Cache error: {0}")]
    CacheError(String),
    #[error("Transformation error: {0}")]
    TransformError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Expired: {0}")]
    Expired(String),
    #[error("Internal server error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, ImageKitError>;

/// Public query parameters for image transformation
#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    pub url: String,
    #[serde(default)]
    pub w: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
    #[serde(default)]
    pub f: Option<ImageFormat>,
    #[serde(default)]
    pub q: Option<u8>,
    #[serde(default)]
    pub t: Option<i64>,
    pub sig: String,
}

// Signing query without `sig`
#[derive(Debug, Deserialize)]
pub struct SignQuery {
    pub url: String,
    #[serde(default)]
    pub w: Option<u32>,
    #[serde(default)]
    pub h: Option<u32>,
    #[serde(default)]
    pub f: Option<ImageFormat>,
    #[serde(default)]
    pub q: Option<u8>,
    #[serde(default)]
    pub t: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SignResponse {
    pub canonical: String,
    pub sig: String,
    pub signed_url: String,
}

fn canonical_params(query_map: &BTreeMap<String, String>) -> String {
    let mut parts = Vec::new();
    for (k, v) in query_map {
        if k != "sig" { parts.push(format!("{}={}", k, v)); }
    }
    parts.join("&")
}

async fn handler(
    Query(query): Query<ImageQuery>,
    state: axum::extract::State<Arc<ImageKitConfig>>,
) -> impl IntoResponse {
    tracing::debug!("Processing image request: url={}, w={:?}, h={:?}, f={:?}, q={:?}", 
                    query.url, query.w, query.h, query.f, query.q);
    
    // Validate and verify signature
    let mut map = BTreeMap::new();
    map.insert("url".into(), query.url.clone());
    if let Some(w) = query.w { map.insert("w".into(), w.to_string()); }
    if let Some(h) = query.h { map.insert("h".into(), h.to_string()); }
    if let Some(f) = query.f { map.insert("f".into(), f.to_string()); }
    if let Some(q) = query.q { map.insert("q".into(), q.to_string()); }
    if let Some(t) = query.t { map.insert("t".into(), t.to_string()); }

    if let Err(e) = verify_signature(&map, &query.sig, &state.secret) {
        tracing::warn!("Signature verification failed for url={}: {:?}", query.url, e);
        let status = match e {
            crate::signature::SignatureError::Expired => StatusCode::GONE,
            _ => StatusCode::UNAUTHORIZED,
        };
        return (status, e.to_string()).into_response();
    }

    // Quality bounds
    if let Some(q) = query.q {
        if q == 0 || q > 100 { return (StatusCode::BAD_REQUEST, "Invalid quality").into_response(); }
    }

    // Build cache and key
    let cache = DiskCache::new(state.cache_dir.clone());
    let canonical_params = canonical_params(&map);
    let key = cache.key_for(&map);

    if let Some(data) = cache.get(&key).await.map_err(|e| e.to_string()).ok().flatten() {
        // Cache hit: return data directly
        tracing::info!("Cache hit for key={}", key);
        METRICS.cache_hits.fetch_add(1, Ordering::Relaxed);  // Track cache hit
        
        let etag = cache.etag_for(&key);
        
        // Determine format from query or default
        let format = query.f.unwrap_or_else(|| state.default_format.unwrap_or(ImageFormat::webp));
        let content_type = match format {
            ImageFormat::webp => "image/webp",
            ImageFormat::jpeg => "image/jpeg",
            ImageFormat::avif => "image/avif",
        };
        
        let mut headers = HeaderMap::new();
        headers.insert("Cache-Control", HeaderValue::from_static(DEFAULT_CACHE_CONTROL));
        headers.insert("ETag", HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")));
        headers.insert(axum::http::header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        return (headers, Body::from(data)).into_response();
    }

    // Cache miss: fetch, transform, cache, stream
    tracing::info!("Cache miss for key={}, fetching from {}", key, query.url);
    METRICS.cache_misses.fetch_add(1, Ordering::Relaxed);  // Track cache miss
    METRICS.transforms.fetch_add(1, Ordering::Relaxed);     // Track transformation
    let max_size = state.max_input_size;
    let allowed = state.allowed_formats.clone();
    let (bytes, _content_type) = match fetch_source(&query.url, max_size, &allowed).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("Failed to fetch {}: {}", query.url, e);
            return (StatusCode::BAD_REQUEST, e.to_string()).into_response();
        }
    };

    let (img, _orig_format) = match decode_image(&bytes) {
        Ok(d) => d,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Decode error: {}", e)).into_response(),
    };

    let resized = match resize_image(img, query.w, query.h) {
        Ok(i) => i,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Resize error: {}", e)).into_response(),
    };

    let target_format = query.f.unwrap_or_else(|| state.default_format.unwrap_or(ImageFormat::webp));
    let quality = query.q.unwrap_or(DEFAULT_QUALITY);

    let encoded = match encode_image(&resized, target_format, quality) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Encode error: {}", e)).into_response(),
    };

    // Store in cache
    if let Err(e) = cache.put(&key, &encoded, target_format, &canonical_params).await {
        tracing::warn!("Failed to cache transformed image: {}", e);
        // Continue anyway - we can still serve the image
    }

    // Return the encoded image directly
    let etag = cache.etag_for(&key);
    let mut headers = HeaderMap::new();
    headers.insert("Cache-Control", HeaderValue::from_static(DEFAULT_CACHE_CONTROL));
    headers.insert("ETag", HeaderValue::from_str(&etag).unwrap_or(HeaderValue::from_static("")));
    let content_type = match target_format {
        ImageFormat::webp => "image/webp",
        ImageFormat::jpeg => "image/jpeg",
        ImageFormat::avif => "image/avif",
    };
    headers.insert(axum::http::header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    (headers, Body::from(encoded)).into_response()
}

async fn sign_handler(
    Query(query): Query<SignQuery>,
    state: axum::extract::State<Arc<ImageKitConfig>>,
) -> Json<SignResponse> {
    let mut map = BTreeMap::new();
    map.insert("url".into(), query.url.clone());
    if let Some(w) = query.w { map.insert("w".into(), w.to_string()); }
    if let Some(h) = query.h { map.insert("h".into(), h.to_string()); }
    if let Some(f) = query.f { map.insert("f".into(), f.to_string()); }
    if let Some(q) = query.q { map.insert("q".into(), q.to_string()); }
    if let Some(t) = query.t { map.insert("t".into(), t.to_string()); }

    let canonical = canonical_params(&map);
    let mut mac = Hmac::<Sha256>::new_from_slice(state.secret.as_bytes()).expect("HMAC key");
    mac.update(canonical.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());

    let mut signed_url = String::from("/img?");
    signed_url.push_str(&canonical);
    signed_url.push_str("&sig=");
    signed_url.push_str(&sig);

    Json(SignResponse { canonical, sig, signed_url })
}

/// Provide an Axum route handler for image transformations.
/// Usage: `app.route("/img", imagekit::route(config))`
pub fn route(config: ImageKitConfig) -> axum::routing::MethodRouter {
    let state = Arc::new(config);
    get(handler).with_state(state)
}

/// Convenience to build a Router with the image route and optional metrics.
async fn upload_handler(
    axum::extract::State(state): axum::extract::State<Arc<ImageKitConfig>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    // Parse multipart fields
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut w: Option<u32> = None;
    let mut h: Option<u32> = None;
    let mut f: Option<ImageFormat> = None;
    let mut q: Option<u8> = None;

    while let Some(field) = match multipart.next_field().await {
        Ok(opt) => opt,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid multipart").into_response(),
    } {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            match field.bytes().await {
                Ok(bytes) => file_bytes = Some(bytes.to_vec()),
                Err(_) => return (StatusCode::BAD_REQUEST, "Invalid file").into_response(),
            }
        } else if name == "w" {
            if let Ok(text) = field.text().await { w = text.parse::<u32>().ok(); }
        } else if name == "h" {
            if let Ok(text) = field.text().await { h = text.parse::<u32>().ok(); }
        } else if name == "f" {
            if let Ok(text) = field.text().await {
                f = match text.as_str() { "jpeg" => Some(ImageFormat::jpeg), "webp" => Some(ImageFormat::webp), "avif" => Some(ImageFormat::avif), _ => None };
            }
        } else if name == "q" {
            if let Ok(text) = field.text().await { q = text.parse::<u8>().ok(); }
        }
    }

    let bytes = match file_bytes { Some(b) => b, None => return (StatusCode::BAD_REQUEST, "Missing file").into_response() };
    let (img, _orig_format) = match decode_image(&bytes) {
        Ok(d) => d,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Decode error: {}", e)).into_response(),
    };

    let resized = match resize_image(img, w, h) {
        Ok(i) => i,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Resize error: {}", e)).into_response(),
    };

    let target_format = f.unwrap_or_else(|| state.default_format.unwrap_or(ImageFormat::webp));
    let quality = q.unwrap_or(DEFAULT_QUALITY);

    let encoded = match encode_image(&resized, target_format, quality) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("Encode error: {}", e)).into_response(),
    };

    let ct = match target_format {
        crate::config::ImageFormat::webp => "image/webp",
        crate::config::ImageFormat::jpeg => "image/jpeg",
        crate::config::ImageFormat::avif => "image/avif",
    };

    let mut headers = HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, HeaderValue::from_static(ct));
    headers.insert("Cache-Control", HeaderValue::from_static(NO_CACHE_CONTROL));
    (headers, Body::from(encoded)).into_response()
}

// ====================================================================================
// OBSERVABILITY - Phase 4
// ====================================================================================

use std::sync::atomic::{AtomicU64, Ordering};

/// Global metrics tracking
pub struct Metrics {
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub transforms: AtomicU64,
    pub errors: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            transforms: AtomicU64::new(0),
            errors: AtomicU64::new(0),
        }
    }
}

lazy_static::lazy_static! {
    static ref METRICS: Metrics = Metrics::new();
}

/// Health check endpoint
async fn health_handler() -> impl IntoResponse {
    use serde_json::json;
    
    Json(json!({
        "status": "healthy",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "imagekit"
    }))
}

/// Cache statistics endpoint
async fn cache_stats_handler(
    axum::extract::State(state): axum::extract::State<Arc<ImageKitConfig>>,
) -> impl IntoResponse {
    use crate::cache::SledCache;
    
    match SledCache::new(&state.cache_dir, state.max_cache_size) {
        Ok(cache) => {
            let stats = cache.stats().await;
            
            // Calculate hit rate
            let hits = METRICS.cache_hits.load(Ordering::Relaxed);
            let misses = METRICS.cache_misses.load(Ordering::Relaxed);
            let total_requests = hits + misses;
            let hit_rate = if total_requests > 0 {
                (hits as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };
            
            use serde_json::json;
            Json(json!({
                "cache": {
                    "total_size_bytes": stats.total_size_bytes,
                    "total_size_mb": stats.total_size_bytes as f64 / 1024.0 / 1024.0,
                    "entry_count": stats.entry_count,
                    "max_size_bytes": stats.max_size_bytes,
                    "max_size_mb": stats.max_size_bytes as f64 / 1024.0 / 1024.0,
                    "usage_percent": (stats.total_size_bytes as f64 / stats.max_size_bytes as f64) * 100.0,
                },
                "requests": {
                    "cache_hits": hits,
                    "cache_misses": misses,
                    "total": total_requests,
                    "hit_rate_percent": hit_rate,
                },
                "transforms": {
                    "total": METRICS.transforms.load(Ordering::Relaxed),
                    "errors": METRICS.errors.load(Ordering::Relaxed),
                }
            })).into_response()
        },
        Err(e) => {
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Cache error: {}", e)).into_response()
        }
    }
}

/// Metrics endpoint (Prometheus-compatible plain text)
async fn metrics_handler() -> impl IntoResponse {
    let hits = METRICS.cache_hits.load(Ordering::Relaxed);
    let misses = METRICS.cache_misses.load(Ordering::Relaxed);
    let transforms = METRICS.transforms.load(Ordering::Relaxed);
    let errors = METRICS.errors.load(Ordering::Relaxed);
    
    let metrics = format!(
        "# HELP imagekit_cache_hits_total Total number of cache hits\n\
         # TYPE imagekit_cache_hits_total counter\n\
         imagekit_cache_hits_total {}\n\
         # HELP imagekit_cache_misses_total Total number of cache misses\n\
         # TYPE imagekit_cache_misses_total counter\n\
         imagekit_cache_misses_total {}\n\
         # HELP imagekit_transforms_total Total number of image transformations\n\
         # TYPE imagekit_transforms_total counter\n\
         imagekit_transforms_total {}\n\
         # HELP imagekit_errors_total Total number of errors\n\
         # TYPE imagekit_errors_total counter\n\
         imagekit_errors_total {}\n",
        hits, misses, transforms, errors
    );
    
    (
        StatusCode::OK,
        [("Content-Type", "text/plain; version=0.0.4")],
        metrics
    )
}

pub fn router(config: ImageKitConfig) -> Router {
    let state = Arc::new(config);
    
    // Observability endpoints - NO rate limiting
    let observability_routes = Router::new()
        .route("/health", get(health_handler))
        .route("/stats/cache", get(cache_stats_handler).with_state(state.clone()))
        .route("/metrics", get(metrics_handler));
    
    // Transformation endpoints - WITH rate limiting
    let mut transform_routes = Router::new()
        .route("/img", get(handler).with_state(state.clone()))
        .route("/upload", axum::routing::post(upload_handler).with_state(state.clone()))
        .route("/sign", get(sign_handler).with_state(state.clone()));
    
    // Only add rate limiting to transformation endpoints if not disabled
    if std::env::var("DISABLE_RATE_LIMIT").is_err() {
        // Configure rate limiting: 10 req/sec per IP, burst of 30
        let governor_conf = Box::new(
            GovernorConfigBuilder::default()
                .per_second(10)
                .burst_size(30)
                .finish()
                .unwrap()
        );
        
        tracing::info!("Router configured with rate limiting: 10/sec, burst 30");
        
        transform_routes = transform_routes.layer(GovernorLayer {
            config: Box::leak(governor_conf),
        });
    } else {
        tracing::info!("Rate limiting disabled");
    }
    
    // Combine routes and add static file serving
    Router::new()
        .merge(observability_routes)
        .merge(transform_routes)
        .nest_service("/", ServeDir::new("frontend"))
}
