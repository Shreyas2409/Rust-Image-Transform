use crate::cache::Cache;
use crate::config::ImageFormat;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use tokio::fs;

/// Disk-based cache implementation (legacy)
/// 
/// Note: This implementation has limitations:
/// - No eviction policy
/// - No size limits
/// - Race conditions possible
/// 
/// Consider using RocksDBCache for production use.
pub struct DiskCache {
    dir: PathBuf
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
    
    fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(key)
    }
    
    pub fn etag_for(&self, key: &str) -> String {
        format!("\"{}\"", key)
    }
    
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
        let p = self.path_for(key);
        match fs::metadata(&p).await {
            Ok(meta) => {
                if meta.is_file() {
                    fs::read(&p).await.map(Some).map_err(|e| e.to_string())
                } else {
                    Ok(None)
                }
            },
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e.to_string())
                }
            },
        }
    }
    
    async fn put(&self, key: &str, bytes: &[u8], format: ImageFormat, _params: &str) -> Result<(), String> {
        if !self.dir.exists() {
            fs::create_dir_all(&self.dir).await.map_err(|e| e.to_string())?;
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
