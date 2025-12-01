use imagekit::transform::{encode_image, resize_image, ImageBytes};
use imagekit::config::ImageFormat;

#[test]
fn resize_and_encode_jpeg() {
    // Create a small RGB image in memory
    let img = image::DynamicImage::new_rgb8(800, 600);
    let resized = resize_image(img, Some(400), None).unwrap();
    let out = encode_image(&resized, ImageFormat::jpeg, 80).unwrap();
    assert!(out.len() > 0);
}

#[test]
fn decode_then_webp() {
    // Generate a simple PNG in memory to test decode path
    let img = image::DynamicImage::new_rgba8(64, 64);
    let mut png = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let (decoded, _) = ImageBytes::decode(&png).unwrap();
    let out = encode_image(&decoded, ImageFormat::webp, 75).unwrap();
    assert!(out.len() > 0);
}