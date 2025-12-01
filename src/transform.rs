use crate::config::ImageFormat;
use crate::ImageKitError;
use image::codecs::avif::AvifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ExtendedColorType};
use image::GenericImageView;
use image::ImageEncoder;

pub struct ImageBytes;

impl ImageBytes {
    pub fn decode(bytes: &[u8]) -> Result<(DynamicImage, Option<ImageFormat>), ImageKitError> {
        let guessed = image::guess_format(bytes).map_err(|e| ImageKitError::TransformError(e.to_string()))?;
        let img = image::load_from_memory_with_format(bytes, guessed)
            .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
        let fmt = match guessed {
            image::ImageFormat::WebP => Some(ImageFormat::webp),
            image::ImageFormat::Jpeg => Some(ImageFormat::jpeg),
            image::ImageFormat::Avif => Some(ImageFormat::avif),
            _ => None,
        };
        Ok((img, fmt))
    }
}

pub fn resize_image(img: DynamicImage, w: Option<u32>, h: Option<u32>) -> Result<DynamicImage, ImageKitError> {
    if w.is_none() && h.is_none() { return Ok(img); }
    let (orig_w, orig_h) = img.dimensions();
    let target_w = w.unwrap_or_else(|| {
        let ratio = h.unwrap() as f32 / orig_h as f32;
        (orig_w as f32 * ratio).round() as u32
    });
    let target_h = h.unwrap_or_else(|| {
        let ratio = w.unwrap() as f32 / orig_w as f32;
        (orig_h as f32 * ratio).round() as u32
    });
    Ok(img.resize(target_w.max(1), target_h.max(1), image::imageops::FilterType::Lanczos3))
}

pub fn encode_image(img: &DynamicImage, fmt: ImageFormat, quality: u8) -> Result<Vec<u8>, ImageKitError> {
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
            // Use webp crate for lossy encoding with quality parameter
            // Quality: 1-100 where higher is better quality but larger file
            let q = quality.clamp(1, 100) as f32;
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            
            // Create WebP encoder with quality setting
            let encoder = webp::Encoder::from_rgb(rgb.as_raw(), w, h);
            let encoded_webp = encoder.encode(q);
            out.extend_from_slice(&encoded_webp);
        }
        ImageFormat::avif => {
            let q = quality.clamp(1, 100);
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let enc = AvifEncoder::new_with_speed_quality(&mut out, 4, q);
            enc.write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
                .map_err(|e| ImageKitError::TransformError(e.to_string()))?;
        }
    }
    Ok(out)
}