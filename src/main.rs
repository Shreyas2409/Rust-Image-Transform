use axum::Router;
use std::net::SocketAddr;
use imagekit::{config::{ImageKitConfig, ImageFormat}, router};

/// ImageKit standalone server entry point.
///
/// Initializes tracing, validates configuration, and starts HTTP server
/// listening for image transformation requests. Designed for cloud deployment
/// with environment-based configuration.
///
/// # Configuration
/// Environment variables:
/// - `IMAGEKIT_SECRET`: HMAC secret for URL signing (required in production)
/// - `PORT`: HTTP listen port (default: 8080)
/// - `RUST_LOG`: Logging verbosity (default: "imagekit=debug,tower_http=debug")
///
/// # Deployment
/// Server binds to 0.0.0.0 to accept external connections, required for
/// platforms like Render, Railway, Fly.io, etc.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging with environment-based filtering
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "imagekit=debug,tower_http=debug".into())
        )
        .init();

    tracing::info!("Starting ImageKit server");

    // Load configuration from environment with fallback defaults
    let cfg = ImageKitConfig {
        secret: std::env::var("IMAGEKIT_SECRET")
            .unwrap_or_else(|_| "local-dev-secret".into()),
        cache_dir: std::path::PathBuf::from("./cache"),
        max_input_size: 8 * 1024 * 1024,        // 8MB prevents DoS
        max_cache_size: Some(10 * 1024 * 1024 * 1024), // 10GB cache limit
        allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
        default_format: Some(ImageFormat::webp), // Best compression/compatibility
    };
    cfg.validate()?;

    let app = Router::new().merge(router(cfg));

    // Cloud platforms inject PORT environment variable
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);

    // Bind to 0.0.0.0 for external access (required for containerized deployment)
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on {}", addr);
    println!("Server listening on {}", addr);
    
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
