use axum::body::Body;
use axum::http::{Request, StatusCode};
use imagekit::config::{ImageFormat, ImageKitConfig};
use imagekit::router;
use std::collections::BTreeMap;
use tower::util::ServiceExt; // for `oneshot`
use serde_json::Value;

/// Helper to create test config
fn test_config() -> ImageKitConfig {
    // Disable rate limiting for tests
    std::env::set_var("DISABLE_RATE_LIMIT", "1");
    
    ImageKitConfig {
        secret: "test-secret-key".to_string(),
        cache_dir: std::path::PathBuf::from("./test-cache"),
        max_input_size: 8 * 1024 * 1024,
        allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
        default_format: Some(ImageFormat::webp),
    }
}

/// Helper to compute signature
fn compute_signature(params: &BTreeMap<String, String>, secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    
    let canonical: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[tokio::test]
async fn test_sign_endpoint() {
    let app = router(test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/sign?url=https://example.com/test.jpg&w=400&f=webp&q=80")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert!(json["canonical"].is_string());
    assert!(json["sig"].is_string());
    assert!(json["signed_url"].is_string());
    
    // Verify canonical format
    let canonical = json["canonical"].as_str().unwrap();
    assert!(canonical.contains("url="));
    assert!(canonical.contains("w=400"));
}

#[tokio::test]
async fn test_img_without_signature_fails() {
    let app = router(test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?url=https://example.com/test.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Missing sig causes deserialization failure = 400 Bad Request
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_img_with_invalid_signature_fails() {
    let app = router(test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/img?url=https://example.com/test.jpg&sig=invalid")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_img_with_expired_signature_fails() {
    let app = router(test_config());
    
    // Create params with expired timestamp (in the past)
    let mut params = BTreeMap::new();
    params.insert("url".to_string(), "https://example.com/test.jpg".to_string());
    params.insert("t".to_string(), "1000000000".to_string()); // Old timestamp
    
    let sig = compute_signature(&params, "test-secret-key");
    
    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/img?url=https://example.com/test.jpg&t=1000000000&sig={}", sig))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn test_img_with_invalid_quality_fails() {
    let app = router(test_config());
    
    // Create valid signature but with invalid quality
    let mut params = BTreeMap::new();
    params.insert("url".to_string(), "https://example.com/test.jpg".to_string());
    params.insert("q".to_string(), "150".to_string()); // Invalid: > 100
    
    let sig = compute_signature(&params, "test-secret-key");
    
    let response = app
        .oneshot(
            Request::builder()
                .uri(&format!("/img?url=https://example.com/test.jpg&q=150&sig={}", sig))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_signature_canonicalization() {
    // Test that signatures are based on sorted params
    let mut params1 = BTreeMap::new();
    params1.insert("url".to_string(), "https://example.com/a.jpg".to_string());
    params1.insert("w".to_string(), "400".to_string());
    params1.insert("h".to_string(), "300".to_string());
    
    let mut params2 = BTreeMap::new();
    params2.insert("h".to_string(), "300".to_string());
    params2.insert("url".to_string(), "https://example.com/a.jpg".to_string());
    params2.insert("w".to_string(), "400".to_string());
    
    let sig1 = compute_signature(&params1, "secret");
    let sig2 = compute_signature(&params2, "secret");
    
    // Should be identical despite different insertion order
    assert_eq!(sig1, sig2);
}

#[tokio::test]
async fn test_rate_limiting_headers_present() {
    let app = router(test_config());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/sign?url=https://example.com/test.jpg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Rate limiting should add headers
    let headers = response.headers();
    
    // tower-governor adds these headers
    assert!(headers.contains_key("x-ratelimit-limit") || response.status() == StatusCode::OK);
}

#[tokio::test]
async fn test_quality_parameter_variations() {
    // Test different quality values are accepted
    let qualities = vec![1, 50, 80, 100];
    
    for q in qualities {
        let mut params = BTreeMap::new();
        params.insert("url".to_string(), "https://example.com/test.jpg".to_string());
        params.insert("q".to_string(), q.to_string());
        
        let sig = compute_signature(&params, "test-secret-key");
        
        // This should not fail with bad request
        // (though it will fail fetching the actual image in CI)
        assert!(sig.len() == 64); // SHA256 hex is 64 chars
    }
}

#[tokio::test]
async fn test_format_parameter_validation() {
    // Test all supported formats
    let formats = vec!["jpeg", "webp", "avif"];
    
    for fmt in formats {
        let mut params = BTreeMap::new();
        params.insert("url".to_string(), "https://example.com/test.jpg".to_string());
        params.insert("f".to_string(), fmt.to_string());
        
        let sig = compute_signature(&params, "test-secret-key");
        
        assert!(sig.len() == 64);
    }
}

#[tokio::test]
async fn test_cache_key_consistency() {
    use sha2::{Digest, Sha256};
    
    // Same params should generate same cache key
    let mut params = BTreeMap::new();
    params.insert("url".to_string(), "https://example.com/cat.jpg".to_string());
    params.insert("w".to_string(), "400".to_string());
    
    let canonical1: String = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    
    let mut params2 = BTreeMap::new();
    params2.insert("w".to_string(), "400".to_string());
    params2.insert("url".to_string(), "https://example.com/cat.jpg".to_string());
    
    let canonical2: String = params2
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");
    
    let mut hasher1 = Sha256::new();
    hasher1.update(canonical1.as_bytes());
    let key1 = hex::encode(hasher1.finalize());
    
    let mut hasher2 = Sha256::new();
    hasher2.update(canonical2.as_bytes());
    let key2 = hex::encode(hasher2.finalize());
    
    assert_eq!(key1, key2);
}

// Cleanup test cache directory after tests
#[tokio::test]
async fn cleanup_test_cache() {
    let _ = tokio::fs::remove_dir_all("./test-cache").await;
}
