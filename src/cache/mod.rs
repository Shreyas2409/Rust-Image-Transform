// Re-export modules
pub mod disk;
pub mod sled_cache;
pub mod cloudflare;

pub use disk::DiskCache;
pub use sled_cache::{SledCache, CacheStats};
pub use cloudflare::{CloudflareCacheConfig, cloudflare_cache_middleware};

use crate::config::ImageFormat;
use std::collections::BTreeMap;

/// Trait for cache backends
#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    /// Generate a cache key from query parameters
    fn key_for(&self, params: &BTreeMap<String, String>) -> String;
    
    /// Get cached data by key
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    
    /// Store data in cache
    async fn put(&self, key: &str, data: &[u8], format: ImageFormat, params: &str) -> Result<(), String>;
}

/// Generate an ETag from a cache key
pub fn etag_for_key(key: &str) -> String {
    format!("\"{}\"", key)
}

/// Generate content type from file extension
pub fn content_type_from_format(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::webp => "image/webp",
        ImageFormat::jpeg => "image/jpeg",
        ImageFormat::avif => "image/avif",
    }
}

/// Detect format from file extension
pub fn format_from_extension(ext: &str) -> Option<ImageFormat> {
    match ext {
        "webp" => Some(ImageFormat::webp),
        "jpeg" | "jpg" => Some(ImageFormat::jpeg),
        "avif" => Some(ImageFormat::avif),
        _ => None,
    }
}
