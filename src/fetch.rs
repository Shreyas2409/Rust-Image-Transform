use crate::{ImageKitError};
use reqwest::Client;
use bytes::BytesMut;
use mime::Mime;
use futures::StreamExt;
use image::GenericImageView;

pub async fn fetch_source(
    url: &str,
    max_size: usize,
    _allowed_formats: &[crate::config::ImageFormat],
) -> Result<(Vec<u8>, String), ImageKitError> {
    let client = Client::new();
    let resp = client.get(url).send().await.map_err(|e| ImageKitError::NetworkError(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ImageKitError::NetworkError(format!("Upstream status: {}", resp.status())));
    }

    // Content-Type validation
    let ct = resp.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if let Ok(m) = ct.parse::<Mime>() {
        if m.type_().as_str() != "image" {
            return Err(ImageKitError::InvalidArgument("Source is not an image".into()));
        }
    } else {
        // If unknown, continue; we'll validate by decoding later
    }

    // Size limit
    if let Some(len) = resp.content_length() {
        if len as usize > max_size {
            return Err(ImageKitError::InvalidArgument("Input exceeds size limit".into()));
        }
    }

    let mut buf = BytesMut::with_capacity(8192);
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await.transpose().map_err(|e| ImageKitError::NetworkError(e.to_string()))? {
        if buf.len() + chunk.len() > max_size {
            return Err(ImageKitError::InvalidArgument("Input exceeds size limit".into()));
        }
        buf.extend_from_slice(&chunk);
    }
    let bytes = buf.to_vec();

    // Dimension validation by decoding header
    match image::guess_format(&bytes)
        .ok()
        .and_then(|fmt| image::load_from_memory_with_format(&bytes, fmt).ok()) {
        Some(img) => {
            let (w, h) = img.dimensions();
            if w == 0 || h == 0 {
                return Err(ImageKitError::InvalidArgument("Invalid image dimensions".into()));
            }
        }
        None => return Err(ImageKitError::InvalidArgument("Unable to decode image for validation".into())),
    }

    Ok((bytes, ct))
}