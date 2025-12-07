use std::path::PathBuf;
use thiserror::Error;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    jpeg,
    webp,
    avif,
}

impl std::fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageFormat::jpeg => write!(f, "jpeg"),
            ImageFormat::webp => write!(f, "webp"),
            ImageFormat::avif => write!(f, "avif"),
        }
    }
}

// Quality constants
pub const DEFAULT_QUALITY: u8 = 80;
pub const MIN_QUALITY: u8 = 1;
pub const MAX_QUALITY: u8 = 100;

// Cache control headers
pub const DEFAULT_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
pub const NO_CACHE_CONTROL: &str = "no-store";


#[derive(Debug, Clone)]
pub struct ImageKitConfig {
    pub secret: String,
    pub cache_dir: PathBuf,
    pub max_input_size: usize, // bytes
    pub max_cache_size: Option<u64>, // bytes - None for unlimited
    pub allowed_formats: Vec<ImageFormat>,
    pub default_format: Option<ImageFormat>,
}

impl Default for ImageKitConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
            cache_dir: PathBuf::from("./cache"),
            max_input_size: 8 * 1024 * 1024,
            max_cache_size: Some(10 * 1024 * 1024 * 1024), // 10GB default
            allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
            default_format: Some(ImageFormat::webp),
        }
    }
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Secret cannot be empty")] EmptySecret,
    #[error("Max input size must be > 0")] InvalidMaxInput,
}

impl ImageKitConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.secret.trim().is_empty() { return Err(ConfigError::EmptySecret); }
        if self.max_input_size == 0 { return Err(ConfigError::InvalidMaxInput); }
        Ok(())
    }
}