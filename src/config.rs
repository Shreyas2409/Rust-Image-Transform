use std::path::PathBuf;
use thiserror::Error;

/// Supported output image formats for transformations.
///
/// Format selection impacts both file size and encoding performance:
/// - JPEG: Fastest encoding, good compression for photos
/// - WebP: Better compression than JPEG, good browser support
/// - AVIF: Best compression, slower encoding, limited browser support
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

/// Default quality setting balancing file size and visual fidelity.
/// Value of 80 provides near-lossless quality for most use cases.
pub const DEFAULT_QUALITY: u8 = 80;

/// Minimum quality threshold to prevent excessive compression artifacts.
pub const MIN_QUALITY: u8 = 1;

/// Maximum quality setting for near-lossless encoding.
pub const MAX_QUALITY: u8 = 100;

/// Aggressive browser cache directive for transformed images.
///
/// 1-year max-age is safe because transformation parameters act as natural
/// cache busters - different parameters yield different cache keys.
pub const DEFAULT_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";

/// Cache bypass directive for dynamic or user-specific content.
pub const NO_CACHE_CONTROL: &str = "no-store";


/// Core configuration for ImageKit transformation service.
///
/// Encapsulates security, caching, and resource limit settings required
/// for production operation. All fields must satisfy validation constraints
/// before service initialization.
#[derive(Debug, Clone)]
pub struct ImageKitConfig {
    /// HMAC secret for URL signature verification.
    /// Must be cryptographically random and kept confidential.
    pub secret: String,
    
    /// Filesystem path for persistent cache storage.
    /// Directory will be created if it doesn't exist.
    pub cache_dir: PathBuf,
    
    /// Maximum input image size in bytes to prevent memory exhaustion.
    /// Requests exceeding this limit are rejected with 413.
    pub max_input_size: usize,
    
    /// Maximum cache size in bytes before LRU eviction begins.
    /// None allows unbounded growth (use with caution).
    pub max_cache_size: Option<u64>,
    
    /// Permitted output formats for transformations.
    /// Restricting formats can improve security and reduce attack surface.
    pub allowed_formats: Vec<ImageFormat>,
    
    /// Default format when client doesn't specify preference.
    /// WebP recommended for balance of compression and compatibility.
    pub default_format: Option<ImageFormat>,
}

impl Default for ImageKitConfig {
    fn default() -> Self {
        Self {
            secret: String::new(),
            cache_dir: PathBuf::from("./cache"),
            max_input_size: 8 * 1024 * 1024,              // 8MB prevents DOS via large uploads
            max_cache_size: Some(10 * 1024 * 1024 * 1024), // 10GB reasonable for most deployments
            allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
            default_format: Some(ImageFormat::webp),       // Best compression/compatibility balance
        }
    }
}

/// Configuration validation errors.
///
/// These errors indicate invalid configuration state that must be
/// corrected before service initialization.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Secret cannot be empty")]
    EmptySecret,
    
    #[error("Max input size must be > 0")]
    InvalidMaxInput,
}

impl ImageKitConfig {
    /// Validates configuration for production readiness.
    ///
    /// Ensures critical security and resource limit settings are properly
    /// configured before service startup. Should be called during initialization.
    ///
    /// # Errors
    /// Returns `ConfigError` if validation constraints are violated.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.secret.trim().is_empty() {
            return Err(ConfigError::EmptySecret);
        }
        if self.max_input_size == 0 {
            return Err(ConfigError::InvalidMaxInput);
        }
        Ok(())
    }
}