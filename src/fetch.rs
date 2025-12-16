use crate::ImageKitError;
use reqwest::Client;
use bytes::BytesMut;
use mime::Mime;
use futures::StreamExt;
use image::GenericImageView;

/// Fetches and validates source image from remote URL.
///
/// Implements defense-in-depth validation strategy:
/// 1. HTTP status code verification
/// 2. Content-Type validation  
/// 3. Content-Length size limits
/// 4. Streaming size enforcement (prevents size header spoofing)
/// 5. Image format validation via decoding
/// 6. Dimension sanity checks
///
/// # Parameters
/// * `url` - Source image URL (must be publicly accessible)
/// * `max_size` - Maximum allowed content size in bytes
/// * `_allowed_formats` - Reserved for future format filtering
///
/// # Security
/// - Prevents memory exhaustion via size limits
/// - Validates actual image data (not just Content-Type)
/// - Streaming download prevents holding large buffers
/// - Rejects malformed or zero-dimension images
///
/// # Returns
/// Tuple of (image_bytes, content_type) on success
///
/// # Errors
/// Returns `ImageKitError` if:
/// - Network request fails or returns non-2xx status
/// - Content-Type is not image/* (when parseable)
/// - Content size exceeds `max_size` limit
/// - Image cannot be decoded or has invalid dimensions
pub async fn fetch_source(
    url: &str,
    max_size: usize,
    _allowed_formats: &[crate::config::ImageFormat],
) -> Result<(Vec<u8>, String), ImageKitError> {
    let client = Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ImageKitError::NetworkError(e.to_string()))?;
        
    if !resp.status().is_success() {
        return Err(ImageKitError::NetworkError(format!(
            "Upstream status: {}",
            resp.status()
        )));
    }

    // Extract and validate Content-Type header
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if let Ok(m) = ct.parse::<Mime>() {
        if m.type_().as_str() != "image" {
            return Err(ImageKitError::InvalidArgument(
                "Source is not an image".into(),
            ));
        }
    }
    // Unknown MIME types continue - will be validated during decode

    // Pre-flight size check based on Content-Length header
    if let Some(len) = resp.content_length() {
        if len as usize > max_size {
            return Err(ImageKitError::InvalidArgument(
                "Input exceeds size limit".into(),
            ));
        }
    }

    // Stream response with size enforcement to prevent header spoofing
    let mut buf = BytesMut::with_capacity(8192);
    let mut stream = resp.bytes_stream();
    
    while let Some(chunk) = stream
        .next()
        .await
        .transpose()
        .map_err(|e| ImageKitError::NetworkError(e.to_string()))?
    {
        if buf.len() + chunk.len() > max_size {
            return Err(ImageKitError::InvalidArgument(
                "Input exceeds size limit".into(),
            ));
        }
        buf.extend_from_slice(&chunk);
    }
    
    let bytes = buf.to_vec();

    // Validate image integrity by attempting decode and dimension check
    match image::guess_format(&bytes)
        .ok()
        .and_then(|fmt| image::load_from_memory_with_format(&bytes, fmt).ok())
    {
        Some(img) => {
            let (w, h) = img.dimensions();
            if w == 0 || h == 0 {
                return Err(ImageKitError::InvalidArgument(
                    "Invalid image dimensions".into(),
                ));
            }
        }
        None => {
            return Err(ImageKitError::InvalidArgument(
                "Unable to decode image for validation".into(),
            ))
        }
    }

    Ok((bytes, ct))
}