use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

/// Supported output image formats
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum Format {
    JPEG,
    WEBP,
    AVIF,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Format::JPEG => write!(f, "JPEG"),
            Format::WEBP => write!(f, "WEBP"),
            Format::AVIF => write!(f, "AVIF"),
        }
    }
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "JPEG" => Ok(Format::JPEG),
            "WEBP" => Ok(Format::WEBP),
            "AVIF" => Ok(Format::AVIF),
            _ => Err(format!("Invalid format: {}", s)),
        }
    }
}

/// Fit modes for image transformation
#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FitMode {
    Cover,
    Contain,
}

impl fmt::Display for FitMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FitMode::Cover => write!(f, "cover"),
            FitMode::Contain => write!(f, "contain"),
        }
    }
}

impl FromStr for FitMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cover" => Ok(FitMode::Cover),
            "contain" => Ok(FitMode::Contain),
            _ => Err(format!("Invalid fit mode: {}", s)),
        }
    }
}

/// Parameters for image transformation parsed from URL query parameters
#[derive(Debug, Deserialize, Clone)]
pub struct TransformParams {
    /// Desired width of the output image
    #[serde(default)]
    pub width: Option<u32>,
    
    /// Desired height of the output image
    #[serde(default)]
    pub height: Option<u32>,
    
    /// Output image format
    #[serde(default)]
    pub format: Option<Format>,
    
    /// Quality of the output image (typically 1-100)
    #[serde(default)]
    pub quality: Option<u8>,
    
    /// Fit mode for image transformation
    #[serde(default, rename = "fit")]
    pub fit_mode: Option<FitMode>,
}

impl FromStr for TransformParams {
    type Err = serde_urlencoded::de::Error;

    /// Parse URL query string into TransformParams structure
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        serde_urlencoded::from_str(s)
    }
}

// Example usage:
// 
// use std::str::FromStr;
//
// fn main() {
//     let query = "width=800&height=600&format=jpeg&quality=80&fit=cover";
//     let params = TransformParams::from_str(query).expect("Failed to parse parameters");
//     println!("Parsed params: {:?}", params);
// }
//
// Note: Ensure that the serde and serde_urlencoded crates are included in Cargo.toml.
// [dependencies]
// serde = { version = "1.0", features = ["derive"] }
// serde_urlencoded = "0.7"
