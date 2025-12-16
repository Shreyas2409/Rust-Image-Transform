use crate::config::ImageFormat;
use crate::ImageKitError;
use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType};
use image::GenericImageView;
use image::ImageEncoder;

/// Decodes raw image bytes into memory-resident representation.
///
/// Performs format detection and validation before decoding to prevent
/// processing malformed images. Supports JPEG, WebP, AVIF, and other
/// formats via the `image` crate.
///
/// # Parameters
/// * `bytes` - Raw encoded image data
///
/// # Returns
/// Tuple of `(DynamicImage, Option<ImageFormat>)` where format is detected
/// when it matches a supported transformation format.
///
/// # Errors
/// Returns `ImageKitError::TransformError` if:
/// - Format cannot be detected from magic bytes
/// - Image data is corrupted or malformed
/// - Decoder encounters unsupported features
pub fn decode_image(bytes: &[u8]) -> Result<(DynamicImage, Option<ImageFormat>), ImageKitError> {
    let guessed = image::guess_format(bytes)
        .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
    
    let img = image::load_from_memory_with_format(bytes, guessed)
        .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
    
    // Map detected format to our supported transformation formats
    let fmt = match guessed {
        image::ImageFormat::WebP => Some(ImageFormat::webp),
        image::ImageFormat::Jpeg => Some(ImageFormat::jpeg),
        image::ImageFormat::Avif => Some(ImageFormat::avif),
        _ => None,
    };
    
    Ok((img, fmt))
}

/// Resizes image maintaining aspect ratio when only one dimension specified.
///
/// Uses Lanczos3 resampling for high-quality output with minimal aliasing.
/// When both dimensions omitted, returns original image unchanged.
///
/// # Parameters
/// * `img` - Source image to resize
/// * `w` - Target width (optional)
/// * `h` - Target height (optional)
///
/// # Behavior
/// - Both specified: Resize to exact dimensions (may distort aspect ratio)
/// - Only width: Scale height proportionally
/// - Only height: Scale width proportionally
/// - Neither: Return original
///
/// Minimum dimension is clamped to 1 pixel to prevent degenerate images.
pub fn resize_image(
    img: DynamicImage,
    w: Option<u32>,
    h: Option<u32>,
) -> Result<DynamicImage, ImageKitError> {
    if w.is_none() && h.is_none() {
        return Ok(img);
    }
    
    let (orig_w, orig_h) = img.dimensions();
    
    // Calculate target dimensions preserving aspect ratio when needed
    let target_w = w.unwrap_or_else(|| {
        let ratio = h.unwrap() as f32 / orig_h as f32;
        (orig_w as f32 * ratio).round() as u32
    });
    
    let target_h = h.unwrap_or_else(|| {
        let ratio = w.unwrap() as f32 / orig_w as f32;
        (orig_h as f32 * ratio).round() as u32
    });
    
    // Lanczos3 provides best quality for downsampling
    Ok(img.resize(
        target_w.max(1),
        target_h.max(1),
        image::imageops::FilterType::Lanczos3,
    ))
}

/// Encodes image to specified format with quality control.
///
/// Format-specific encoding strategies:
/// - **JPEG**: RGB color space, DCT-based lossy compression
/// - **WebP**: RGB lossy encoding via libwebp
/// - **AVIF**: RGBA with AV1 compression (slowest, best compression)
///
/// # Parameters
/// * `img` - Image to encode
/// * `fmt` - Target output format
/// * `quality` - Compression quality (1-100, higher = better quality/larger file)
///
/// # Performance
/// Relative encoding speed (typical): JPEG > WebP > AVIF
/// Quality is automatically clamped to valid range [1, 100].
///
/// # Returns
/// Encoded image bytes ready for transmission or storage.
///
/// # Errors
/// Returns `ImageKitError::TransformError` on encoder failures.
pub fn encode_image(
    img: &DynamicImage,
    fmt: ImageFormat,
    quality: u8,
) -> Result<Vec<u8>, ImageKitError> {
    let mut out = Vec::new();
    
    match fmt {
        ImageFormat::jpeg => {
            let q = quality.clamp(1, 100);
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let enc = JpegEncoder::new_with_quality(&mut out, q);
            enc.write_image(rgb.as_raw(), w, h, ExtendedColorType::Rgb8)
                .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
        }
        ImageFormat::webp => {
            let q = quality.clamp(1, 100) as f32;
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            
            let encoder = webp::Encoder::from_rgb(rgb.as_raw(), w, h);
            let encoded_webp = encoder.encode(q);
            out.extend_from_slice(&encoded_webp);
        }
        ImageFormat::avif => {
            let q = quality.clamp(1, 100);
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            // Speed 4 balances encoding time and compression ratio
            let enc = AvifEncoder::new_with_speed_quality(&mut out, 4, q);
            enc.write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
        }
    }
    
    Ok(out)
}