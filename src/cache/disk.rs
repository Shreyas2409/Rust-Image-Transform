use crate::cache::Cache;
use crate::config::ImageFormat;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use tokio::fs;

/// Simple filesystem-based cache implementation.
///
/// **Production Warning:** This implementation has significant limitations:
/// - No automatic eviction policy (unbounded growth)
/// - No size tracking or limits
/// - Potential race conditions on concurrent writes
/// - No atomic operations or file locking
///
/// Suitable for:
/// - Development and testing environments
/// - Low-traffic deployments with manual cache management
/// - Temporary caching with external cleanup processes
///
/// **Recommendation:** Use `SledCache` for production deployments requiring:
/// - Automatic LRU eviction
/// - Size limits and tracking
/// - Better concurrency handling
/// - Atomic operations
pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    /// Creates new disk cache instance at specified directory.
    ///
    /// Directory will be created automatically on first write if it doesn't exist.
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
    
    /// Computes filesystem path for cache key.
    ///
    /// Keys are used directly as filenames (after hex encoding),
    /// with format extension appended during storage.
    fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }
    
    /// Generates ETag header value from cache key.
    ///
    /// Simple quoted-string format per RFC 7232.
    /// In production, consider including modification time or content hash.
    pub fn etag_for(&self, key: &str) -> String {
        format!("\"{}\"", key)
    }
    
    /// Determines Content-Type from file extension.
    ///
    /// Returns appropriate MIME type for supported image formats.
    /// Used when serving cached files directly.
    pub fn content_type_for_path(&self, path: &Path) -> Option<String> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("webp") => Some("image/webp".into()),
            Some("jpeg") | Some("jpg") => Some("image/jpeg".into()),
            Some("avif") => Some("image/avif".into()),
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl Cache for DiskCache {
    /// Generates deterministic cache key from transformation parameters.
    ///
    /// Uses SHA-256 hash of canonical parameter string to produce
    /// collision-resistant keys with uniform distribution. Parameter order
    /// is normalized via BTreeMap iteration.
    fn key_for(&self, params: &BTreeMap<String, String>) -> String {
        let canonical: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
            
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }
    
    /// Retrieves cached data if present.
    ///
    /// Returns `None` if key doesn't exist (cache miss).
    /// Propagates filesystem errors other than NotFound.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let p = self.path_for(key);
        match fs::metadata(&p).await {
            Ok(meta) => {
                if meta.is_file() {
                    fs::read(&p).await.map(Some).map_err(|e| e.to_string())
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e.to_string())
                }
            }
        }
    }
    
    /// Stores transformed image data in cache.
    ///
    /// Creates cache directory if it doesn't exist. Filename includes
    /// format extension for easier manual inspection and debugging.
    ///
    /// **Warning:** No file locking - concurrent writes to same key may corrupt data.
    async fn put(
        &self,
        key: &str,
        bytes: &[u8],
        format: ImageFormat,
        _params: &str,
    ) -> Result<(), String> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir)
                .await
                .map_err(|e| e.to_string())?;
        }
        
        let ext = match format {
            ImageFormat::webp => "webp",
            ImageFormat::jpeg => "jpeg",
            ImageFormat::avif => "avif",
        };
        
        let filename = format!("{}.{}", key, ext);
        let path = self.dir.join(filename);
        fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}
