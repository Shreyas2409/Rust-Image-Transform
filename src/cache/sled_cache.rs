use crate::cache::Cache;
use crate::config::ImageFormat;
use sled::Db;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Default maximum cache size: 10GB
pub const DEFAULT_MAX_CACHE_SIZE: u64 = 10 * 1024 * 1024 * 1024;

/// Metadata stored alongside cached images
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheMetadata {
    pub key: String,
    pub format: ImageFormat,
    pub size: usize,
    pub created_at: u64,
    pub accessed_at: u64,
    pub params: String, // JSON-serialized params for debugging
}

/// Statistics about the cache
#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub total_size_bytes: u64,
    pub entry_count: usize,
    pub max_size_bytes: u64,
    pub hit_rate: Option<f64>,
}

/// Sled-based cache with LRU eviction
/// 
/// This cache provides:
/// - Persistent storage with automatic eviction
/// - LRU (Least Recently Used) eviction policy
/// - Metadata tracking for debugging and analytics
/// - Atomic operations
/// - Configurable size limits
/// - Pure Rust (no C++ compilation needed)
pub struct SledCache {
    db: Db,
    max_size: u64,
}

impl SledCache {
    /// Create a new Sled cache
    ///
    /// # Arguments
    /// * `path` - Directory to store the Sled database
    /// * `max_size` - Optional maximum size in bytes (default: 10GB)
    pub fn new(path: impl AsRef<Path>, max_size: Option<u64>) -> Result<Self, String> {
        let db = sled::open(path).map_err(|e| format!("Failed to open Sled database: {}", e))?;
        
        Ok(Self {
            db,
            max_size: max_size.unwrap_or(DEFAULT_MAX_CACHE_SIZE),
        })
    }
    
    /// Generate metadata key from cache key
    fn metadata_key(key: &str) -> String {
        format!("meta:{}", key)
    }
    
    /// Generate data key from cache key
    fn data_key(key: &str) -> String {
        format!("data:{}", key)
    }
    
    /// Get current total size of cached data
    async fn current_size(&self) -> u64 {
        let mut total = 0u64;
        
        for item in self.db.iter() {
            if let Ok((key, value)) = item {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    if key_str.starts_with("meta:") {
                        if let Ok(meta) = serde_json::from_slice::<CacheMetadata>(&value) {
                            total += meta.size as u64;
                        }
                    }
                }
            }
        }
        
        total
    }
    
    /// Evict least recently used entries until under size limit
    async fn evict_if_needed(&self) -> Result<(), String> {
        let current = self.current_size().await;
        
        if current <= self.max_size {
            return Ok(());
        }
        
        tracing::info!("Cache size {} exceeds limit {}, starting eviction", current, self.max_size);
        
        // Collect all metadata entries
        let mut entries: Vec<CacheMetadata> = Vec::new();
        
        for item in self.db.iter() {
            if let Ok((key, value)) = item {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    if key_str.starts_with("meta:") {
                        if let Ok(meta) = serde_json::from_slice::<CacheMetadata>(&value) {
                            entries.push(meta);
                        }
                    }
                }
            }
        }
        
        // Sort by access time (oldest first) - LRU eviction
        entries.sort_by_key(|e| e.accessed_at);
        
        // Remove entries until we're under target size (90% of max)
        let mut freed = 0u64;
        let target_to_free = current.saturating_sub(self.max_size * 90 / 100);
        let mut evicted_count = 0;
        
        for entry in entries {
            if freed >= target_to_free {
                break;
            }
            
            // Delete both metadata and data
            self.db.remove(Self::metadata_key(&entry.key).as_bytes())
                .map_err(|e| e.to_string())?;
            self.db.remove(Self::data_key(&entry.key).as_bytes())
                .map_err(|e| e.to_string())?;
            
            freed += entry.size as u64;
            evicted_count += 1;
            
            tracing::debug!("Evicted cache entry: key={}, size={}, age={}", 
                           entry.key, entry.size, 
                           SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - entry.accessed_at);
        }
        
        self.db.flush().map_err(|e| e.to_string())?;
        
        tracing::info!("Eviction complete: freed {} bytes by removing {} entries", freed, evicted_count);
        
        Ok(())
    }
    
    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let size = self.current_size().await;
        let mut count = 0;
        
        for item in self.db.iter() {
            if let Ok((key, _)) = item {
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    if key_str.starts_with("meta:") {
                        count += 1;
                    }
                }
            }
        }
        
        CacheStats {
            total_size_bytes: size,
            entry_count: count,
            max_size_bytes: self.max_size,
            hit_rate: None, // TODO: Track hits/misses for this
        }
    }
}

#[async_trait::async_trait]
impl Cache for SledCache {
    fn key_for(&self, params: &BTreeMap<String, String>) -> String {
        let canonical: String = params.iter()
            .map(|(k,v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }
    
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let data_key = Self::data_key(key);
        let meta_key = Self::metadata_key(key);
        
        // Get data
        let data = match self.db.get(data_key.as_bytes()).map_err(|e| e.to_string())? {
            Some(d) => d.to_vec(),
            None => return Ok(None),
        };
        
        // Update access time (cache hit)
        if let Some(meta_bytes) = self.db.get(meta_key.as_bytes()).map_err(|e| e.to_string())? {
            if let Ok(mut meta) = serde_json::from_slice::<CacheMetadata>(&meta_bytes[..]) {
                meta.accessed_at = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                // Write back updated metadata
                let _ = self.db.insert(
                    meta_key.as_bytes(),
                    serde_json::to_vec(&meta).unwrap()
                );
            }
        }
        
        Ok(Some(data))
    }
    
    async fn put(
        &self,
        key: &str,
        data: &[u8],
        format: ImageFormat,
        params: &str
    ) -> Result<(), String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        let metadata = CacheMetadata {
            key: key.to_string(),
            format,
            size: data.len(),
            created_at: now,
            accessed_at: now,
            params: params.to_string(),
        };
        
        // Store data
        self.db.insert(
            Self::data_key(key).as_bytes(),
            data
        ).map_err(|e| format!("Failed to write cache data: {}", e))?;
        
        // Store metadata
        self.db.insert(
            Self::metadata_key(key).as_bytes(),
            serde_json::to_vec(&metadata).unwrap()
        ).map_err(|e| format!("Failed to write cache metadata: {}", e))?;
        
        // Flush to disk
        self.db.flush().map_err(|e| e.to_string())?;
        
        // Check if eviction needed
        self.evict_if_needed().await?;
        
        Ok(())
    }
}
