use crate::config::ImageFormat;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use tokio::fs;

#[async_trait::async_trait]
pub trait Cache: Send + Sync {
    fn key_for(&self, params: &BTreeMap<String, String>) -> String;
    fn etag_for(&self, key: &str) -> String;
    fn content_type_for_path(&self, path: &Path) -> Option<String>;
    async fn get(&self, key: &str) -> Result<Option<PathBuf>, String>;
    async fn put(&self, key: &str, bytes: &[u8], format: ImageFormat) -> Result<PathBuf, String>;
}

pub struct DiskCache { dir: PathBuf }

impl DiskCache {
    pub fn new(dir: PathBuf) -> Self { Self { dir } }
    fn path_for(&self, key: &str) -> PathBuf { self.dir.join(key) }
}

#[async_trait::async_trait]
impl Cache for DiskCache {
    fn key_for(&self, params: &BTreeMap<String, String>) -> String {
        let canonical: String = params.iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join("&");
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }

    fn etag_for(&self, key: &str) -> String { format!("\"{}\"", key) }

    fn content_type_for_path(&self, path: &Path) -> Option<String> {
        match path.extension().and_then(|e| e.to_str()) {
            Some("webp") => Some("image/webp".into()),
            Some("jpeg") | Some("jpg") => Some("image/jpeg".into()),
            Some("avif") => Some("image/avif".into()),
            _ => None,
        }
    }

    async fn get(&self, key: &str) -> Result<Option<PathBuf>, String> {
        let p = self.path_for(key);
        match fs::metadata(&p).await {
            Ok(meta) => if meta.is_file() { Ok(Some(p)) } else { Ok(None) },
            Err(e) => if e.kind() == std::io::ErrorKind::NotFound { Ok(None) } else { Err(e.to_string()) },
        }
    }

    async fn put(&self, key: &str, bytes: &[u8], format: ImageFormat) -> Result<PathBuf, String> {
        if !self.dir.exists() { fs::create_dir_all(&self.dir).await.map_err(|e| e.to_string())?; }
        // Use key as filename with extension
        let ext = match format {
            ImageFormat::webp => "webp",
            ImageFormat::jpeg => "jpeg",
            ImageFormat::avif => "avif",
        };
        let filename = format!("{}.{}", key, ext);
        let path = self.dir.join(filename);
        fs::write(&path, bytes).await.map_err(|e| e.to_string())?;
        Ok(path)
    }
}