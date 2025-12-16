use axum::{
    http::{header, HeaderValue, Request, Response},
    middleware::Next,
    body::Body,
};

/// Configuration for Cloudflare-compatible HTTP caching directives.
///
/// This struct encapsulates cache control settings optimized for Cloudflare's CDN,
/// supporting both edge and browser caching with configurable TTLs and stale content policies.
#[derive(Clone, Debug)]
pub struct CloudflareCacheConfig {
    /// CDN edge cache time-to-live in seconds (via s-maxage directive).
    /// Controls how long Cloudflare caches content at edge locations.
    pub edge_max_age: u32,
    
    /// Browser cache time-to-live in seconds (via max-age directive).
    /// Controls client-side cache duration. Safe to set high for cache-busted content.
    pub browser_max_age: u32,
    
    /// Enable public caching by intermediaries (CDNs, proxies).
    /// Set false only for user-specific or sensitive content.
    pub public: bool,
    
    /// Serve stale content duration when origin is unreachable (stale-if-error).
    /// Improves resilience during origin server failures.
    pub stale_if_error: Option<u32>,
    
    /// Serve stale content while revalidating in background (stale-while-revalidate).
    /// Optimizes perceived performance by serving cached content immediately.
    pub stale_while_revalidate: Option<u32>,
    
    /// Mark content as immutable for the duration of its cache lifetime.
    /// Enables aggressive caching for content-addressed or versioned resources.
    pub immutable: bool,
}

impl Default for CloudflareCacheConfig {
    fn default() -> Self {
        Self {
            edge_max_age: 86400,              // 1 day: balances freshness and cache efficiency
            browser_max_age: 31536000,        // 1 year: safe due to cache-busting via query parameters
            public: true,
            stale_if_error: Some(86400),      // Maintain availability during origin failures
            stale_while_revalidate: Some(60), // Optimize response time with background updates
            immutable: true,                  // Transformation parameters guarantee content uniqueness
        }
    }
}

impl CloudflareCacheConfig {
    /// Returns optimal configuration for transformed image assets.
    ///
    /// Leverages transformation parameters as natural cache busters,
    /// enabling aggressive browser caching while maintaining edge freshness.
    pub fn for_images() -> Self {
        Self::default()
    }
    
    /// Creates configuration for short-lived dynamic content.
    ///
    /// # Arguments
    /// * `ttl_seconds` - Unified TTL for both edge and browser caches
    pub fn for_dynamic(ttl_seconds: u32) -> Self {
        Self {
            edge_max_age: ttl_seconds,
            browser_max_age: ttl_seconds,
            public: true,
            stale_if_error: Some(ttl_seconds * 2),
            stale_while_revalidate: Some(60),
            immutable: false,
        }
    }
    
    /// Creates configuration that completely bypasses all caching layers.
    ///
    /// Use only for highly sensitive or personalized content where
    /// caching could lead to data leakage or stale responses.
    pub fn no_cache() -> Self {
        Self {
            edge_max_age: 0,
            browser_max_age: 0,
            public: false,
            stale_if_error: None,
            stale_while_revalidate: None,
            immutable: false,
        }
    }
    
    /// Generates RFC 7234 compliant Cache-Control header value.
    ///
    /// Combines directives optimized for Cloudflare's caching behavior,
    /// including separate TTLs for browser (max-age) and CDN (s-maxage).
    pub fn cache_control_value(&self) -> String {
        if self.edge_max_age == 0 {
            return "no-store, no-cache, must-revalidate".to_string();
        }
        
        let mut parts = Vec::new();
        
        parts.push(if self.public { "public" } else { "private" }.to_string());
        
        // Browser cache duration
        parts.push(format!("max-age={}", self.browser_max_age));
        
        // CDN cache duration (takes precedence over max-age for shared caches)
        parts.push(format!("s-maxage={}", self.edge_max_age));
        
        if self.immutable {
            parts.push("immutable".to_string());
        }
        
        if let Some(seconds) = self.stale_if_error {
            parts.push(format!("stale-if-error={}", seconds));
        }
        
        if let Some(seconds) = self.stale_while_revalidate {
            parts.push(format!("stale-while-revalidate={}", seconds));
        }
        
        parts.join(", ")
    }
    
    /// Generates Cloudflare-specific CDN-Cache-Control header value.
    ///
    /// This proprietary header provides independent cache control for Cloudflare's
    /// edge network without affecting downstream caches or browsers.
    pub fn cdn_cache_control_value(&self) -> String {
        if self.edge_max_age == 0 {
            return "no-store".to_string();
        }
        
        format!("max-age={}", self.edge_max_age)
    }
}

/// Axum middleware that injects Cloudflare-optimized caching headers.
///
/// Automatically applies cache directives to successful responses (2xx status codes),
/// configuring both standard HTTP caching and Cloudflare-specific extensions.
///
/// # Behavior
/// - Only modifies successful responses to avoid caching error states
/// - Sets Cache-Control with dual TTLs for browser and edge caching
/// - Adds CDN-Cache-Control for Cloudflare-specific configuration
/// - Includes Vary: Accept-Encoding to support compression negotiation
pub async fn cloudflare_cache_middleware(
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let mut response = next.run(req).await;
    
    if response.status().is_success() {
        let config = CloudflareCacheConfig::for_images();
        
        if let Ok(value) = HeaderValue::from_str(&config.cache_control_value()) {
            response.headers_mut().insert(header::CACHE_CONTROL, value);
        }
        
        if let Ok(value) = HeaderValue::from_str(&config.cdn_cache_control_value()) {
            response.headers_mut().insert(
                header::HeaderName::from_static("cdn-cache-control"),
                value,
            );
        }
        
        // Enable cache variance based on compression negotiation
        if let Ok(value) = HeaderValue::from_str("Accept-Encoding") {
            response.headers_mut().insert(header::VARY, value);
        }
    }
    
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_cache_control() {
        let config = CloudflareCacheConfig::default();
        let value = config.cache_control_value();
        
        assert!(value.contains("public"));
        assert!(value.contains("max-age=31536000"));
        assert!(value.contains("s-maxage=86400"));
        assert!(value.contains("immutable"));
        assert!(value.contains("stale-if-error=86400"));
        assert!(value.contains("stale-while-revalidate=60"));
    }
    
    #[test]
    fn test_no_cache() {
        let config = CloudflareCacheConfig::no_cache();
        let value = config.cache_control_value();
        
        assert_eq!(value, "no-store, no-cache, must-revalidate");
    }
    
    #[test]
    fn test_dynamic_cache() {
        let config = CloudflareCacheConfig::for_dynamic(3600);
        let value = config.cache_control_value();
        
        assert!(value.contains("max-age=3600"));
        assert!(value.contains("s-maxage=3600"));
        assert!(!value.contains("immutable"));
    }
    
    #[test]
    fn test_cdn_cache_control() {
        let config = CloudflareCacheConfig::default();
        let value = config.cdn_cache_control_value();
        
        assert_eq!(value, "max-age=86400");
    }
}
