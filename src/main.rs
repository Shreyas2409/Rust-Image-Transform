use axum::Router;
use std::net::SocketAddr;
use imagekit::{config::{ImageKitConfig, ImageFormat}, router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for observability
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "imagekit=debug,tower_http=debug".into())
        )
        .init();

    tracing::info!("Starting ImageKit server");

    // Example server with /img route
    let cfg = ImageKitConfig {
        secret: std::env::var("IMAGEKIT_SECRET").unwrap_or_else(|_| "local-dev-secret".into()),
        cache_dir: std::path::PathBuf::from("./cache"),
        max_input_size: 8 * 1024 * 1024,
        allowed_formats: vec![ImageFormat::jpeg, ImageFormat::webp, ImageFormat::avif],
        default_format: Some(ImageFormat::webp),
    };
    cfg.validate()?;

    let app = Router::new()
        .merge(router(cfg));

    // Read port from environment (for Render, Railway, etc.) or default to 8080
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .unwrap_or(8080);

    // Bind to 0.0.0.0 to accept external connections (required for cloud deployment)
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Server listening on {}", addr);
    println!("Server listening on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
