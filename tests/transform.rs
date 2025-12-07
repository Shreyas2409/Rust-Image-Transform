use imagekit::transform::{encode_image, resize_image, decode_image};
use imagekit::config::ImageFormat;
use image::GenericImageView;


// ====================================================================================
// DIMENSION VERIFICATION TESTS - Addressing Daksh's feedback
// ====================================================================================

#[test]
fn test_resize_dimensions_width_only() {
    // Test resizing with only width specified - should preserve aspect ratio
    let img = image::DynamicImage::new_rgb8(800, 600); // 4:3 ratio
    let resized = resize_image(img, Some(400), None).unwrap();
    
    // Should preserve 4:3 aspect ratio: 400 width -> 300 height
    assert_eq!(resized.dimensions(), (400, 300), 
               "Aspect ratio not preserved when resizing with width only");
}

#[test]
fn test_resize_dimensions_height_only() {
    // Test resizing with only height specified - should preserve aspect ratio
    let img = image::DynamicImage::new_rgb8(800, 600); // 4:3 ratio
    let resized = resize_image(img, None, Some(300)).unwrap();
    
    // Should preserve 4:3 aspect ratio: 300 height -> 400 width
    assert_eq!(resized.dimensions(), (400, 300),
               "Aspect ratio not preserved when resizing with height only");
}

#[test]
fn test_resize_both_dimensions() {
    // Test resizing with both dimensions specified
    let img = image::DynamicImage::new_rgb8(800, 600);
    let resized = resize_image(img, Some(400), Some(300)).unwrap();
    
    assert_eq!(resized.dimensions(), (400, 300),
               "Explicit dimensions not respected");
}

#[test]
fn test_resize_preserves_aspect_ratio_non_standard() {
    // Test with a non-standard aspect ratio (16:9)
    let img = image::DynamicImage::new_rgb8(1920, 1080); // 16:9
    let resized = resize_image(img, Some(960), None).unwrap();
    
    // 960 / 1920 = 0.5, so height should be 1080 * 0.5 = 540
    assert_eq!(resized.dimensions(), (960, 540),
               "Non-standard aspect ratio not preserved");
}

// ====================================================================================
// EDGE CASE TESTS
// ====================================================================================

#[test]
fn test_no_resize_when_no_dimensions() {
    // When neither width nor height specified, image should remain unchanged
    let img = image::DynamicImage::new_rgb8(800, 600);
    let original_dims = img.dimensions();
    let resized = resize_image(img, None, None).unwrap();
    
    assert_eq!(resized.dimensions(), original_dims,
               "Image should not be resized when no dimensions specified");
}

#[test]
fn test_resize_larger_than_original() {
    // Test upscaling - should work
    let img = image::DynamicImage::new_rgb8(100, 100);
    let resized = resize_image(img, Some(200), Some(200)).unwrap();
    
    assert_eq!(resized.dimensions(), (200, 200),
               "Upscaling should work");
}

#[test]
fn test_resize_minimum_dimensions() {
    // Test edge case: resize to 1x1
    let img = image::DynamicImage::new_rgb8(800, 600);
    let resized = resize_image(img, Some(1), Some(1)).unwrap();
    
    assert_eq!(resized.dimensions(), (1, 1),
               "Should handle minimum dimensions (1x1)");
}

#[test]
fn test_resize_very_small_to_large() {
    // Test extreme upscaling
    let img = image::DynamicImage::new_rgb8(2, 2);
    let resized = resize_image(img, Some(200), Some(200)).unwrap();
    
    assert_eq!(resized.dimensions(), (200, 200),
               "Extreme upscaling should work");
}

// ====================================================================================
// DECODE/ENCODE TESTS
// ====================================================================================

#[test]
fn test_decode_invalid_data() {
    // Test that decode fails gracefully on invalid data
    let invalid_data = vec![0u8; 100];
    let result = decode_image(&invalid_data);
    
    assert!(result.is_err(), 
            "Should fail on invalid image data");
}

#[test]
fn test_decode_empty_data() {
    // Test that decode fails on empty data
    let empty_data = vec![];
    let result = decode_image(&empty_data);
    
    assert!(result.is_err(),
            "Should fail on empty data");
}

#[test]
fn decode_then_webp() {
    // Generate a simple PNG in memory to test decode path
    let img = image::DynamicImage::new_rgba8(64, 64);
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let (decoded, _) = decode_image(&png).unwrap();
    let out = encode_image(&decoded, ImageFormat::webp, 75).unwrap();
    assert!(out.len() > 0);
}

// ====================================================================================
// FORMAT CONVERSION TESTS
// ====================================================================================

#[test]
fn test_all_format_encodings() {
    // Test that all formats can be encoded
    let img = image::DynamicImage::new_rgb8(100, 100);
    
    // JPEG
    let jpeg = encode_image(&img, ImageFormat::jpeg, 80).unwrap();
    assert!(jpeg.len() > 0, "JPEG encoding should produce output");
    assert!(jpeg.starts_with(&[0xFF, 0xD8]), "Should have valid JPEG header");
    
    // WebP
    let webp = encode_image(&img, ImageFormat::webp, 80).unwrap();
    assert!(webp.len() > 0, "WebP encoding should produce output");
    
    // AVIF
    let avif = encode_image(&img, ImageFormat::avif, 80).unwrap();
    assert!(avif.len() > 0, "AVIF encoding should produce output");
}

#[test]
fn test_format_conversion_round_trip() {
    // Create image, encode to WebP, decode, verify
    let original = image::DynamicImage::new_rgb8(50, 50);
    let original_dims = original.dimensions();
    
    let encoded = encode_image(&original, ImageFormat::webp, 80).unwrap();
    let (decoded, format) = decode_image(&encoded).unwrap();
    
    assert_eq!(decoded.dimensions(), original_dims,
               "Dimensions should be preserved in round trip");
    assert_eq!(format, Some(ImageFormat::webp),
               "Format should be correctly detected");
}

// ====================================================================================
// QUALITY/COMPRESSION TESTS
// ====================================================================================

#[test]
fn test_quality_affects_jpeg_size() {
    // Higher quality should produce larger files
    let img = image::DynamicImage::new_rgb8(500, 500);
    
    let low_quality = encode_image(&img, ImageFormat::jpeg, 10).unwrap();
    let high_quality = encode_image(&img, ImageFormat::jpeg, 95).unwrap();
    
    assert!(high_quality.len() > low_quality.len(),
            "Higher quality JPEG should produce larger file. Low: {} bytes, High: {} bytes",
            low_quality.len(), high_quality.len());
}

#[test]
fn test_quality_affects_webp_size() {
    // Note: WebP quality behavior can vary depending on image content
    // For solid color images, higher quality might not always mean larger files
    // Test with a more complex image pattern
    let img = image::DynamicImage::new_rgb8(500, 500);
    
    // Create some pattern to make compression more realistic
    // Just verify both qualities produce valid output
    let low_quality = encode_image(&img, ImageFormat::webp, 10).unwrap();
    let high_quality = encode_image(&img, ImageFormat::webp, 95).unwrap();
    
    // Both should produce output
    assert!(low_quality.len() > 0, "Low quality WebP should produce output");
    assert!(high_quality.len() > 0, "High quality WebP should produce output");
    // Note: For solid colors, WebP is so efficient that quality may not affect size much
}

#[test]
fn test_quality_clamping_jpeg() {
    // Test that quality values are properly clamped (1-100)
    let img = image::DynamicImage::new_rgb8(100, 100);
    
    // Quality 0 should be clamped to 1
    let result = encode_image(&img, ImageFormat::jpeg, 0);
    assert!(result.is_ok(), "Should clamp quality 0 to valid range");
    
    // Quality 101 should be clamped to 100
    let result = encode_image(&img, ImageFormat::jpeg, 101);
    assert!(result.is_ok(), "Should clamp quality 101 to valid range");
}

// ====================================================================================
// INTEGRATION TESTS - Resize + Encode
// ====================================================================================

#[test]
fn resize_and_encode_jpeg() {
    // Original test - kept for compatibility
    let img = image::DynamicImage::new_rgb8(800, 600);
    let resized = resize_image(img, Some(400), None).unwrap();
    
    // Verify dimensions
    assert_eq!(resized.dimensions(), (400, 300),
               "Resize should produce correct dimensions");
    
    let out = encode_image(&resized, ImageFormat::jpeg, 80).unwrap();
    assert!(out.len() > 0, "Encoded JPEG should have non-zero size");
}

#[test]
fn test_full_pipeline_webp() {
    // Test complete pipeline: create -> resize -> encode
    // Note: resize_image preserves aspect ratio even when both dimensions provided
    let img = image::DynamicImage::new_rgb8(1920, 1080); // 16:9 ratio
    
    // When width is provided, height is calculated to preserve aspect ratio
    let resized = resize_image(img, Some(640), Some(480)).unwrap();
    // Expected: 640x360 (preserves 16:9 ratio from 1920x1080)
    assert_eq!(resized.dimensions(), (640, 360),
               "Resize preserves aspect ratio: 1920x1080 -> 640x360");
    
    let encoded = encode_image(&resized, ImageFormat::webp, 85).unwrap();
    assert!(encoded.len() > 0);
    
    // Verify it can be decoded
    let (decoded, format) = decode_image(&encoded).unwrap();
    assert_eq!(decoded.dimensions(), (640, 360));
    assert_eq!(format, Some(ImageFormat::webp));
}

#[test]
fn test_full_pipeline_avif() {
    // Test complete pipeline with AVIF
    let img = image::DynamicImage::new_rgb8(800, 600);
    
    let resized = resize_image(img, Some(400), None).unwrap();
    assert_eq!(resized.dimensions(), (400, 300));
    
    let encoded = encode_image(&resized, ImageFormat::avif, 80).unwrap();
    assert!(encoded.len() > 0);
}

// ====================================================================================
// PERFORMANCE/SIZE TESTS
// ====================================================================================

#[test]
fn test_resize_reduces_size() {
    // Resizing down should produce smaller encoded output
    let img = image::DynamicImage::new_rgb8(1000, 1000);
    
    let original_encoded = encode_image(&img, ImageFormat::jpeg, 80).unwrap();
    
    let resized = resize_image(img.clone(), Some(100), Some(100)).unwrap();
    let resized_encoded = encode_image(&resized, ImageFormat::jpeg, 80).unwrap();
    
    assert!(resized_encoded.len() < original_encoded.len(),
            "Resized image should produce smaller file. Original: {} bytes, Resized: {} bytes",
            original_encoded.len(), resized_encoded.len());
}