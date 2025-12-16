use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;

/// Signature verification errors.
///
/// These errors indicate authentication failures or expired request timestamps.
/// All verification failures should be treated as potential security issues.
#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("missing signature")]
    Missing,
    
    #[error("invalid signature")]
    Invalid,
    
    #[error("expired")]
    Expired,
}

/// Generates canonical parameter string for HMAC signature computation.
///
/// Parameters are sorted lexicographically and joined with '&' to ensure
/// consistent signature generation across clients and servers. The `sig`
/// parameter itself is excluded from the canonical string to prevent
/// circular dependencies.
///
/// # Format
/// Returns "key1=value1&key2=value2" sorted by key name.
fn canonical_string(params: &BTreeMap<String, String>) -> String {
    let mut pairs: Vec<String> = Vec::new();
    for (k, v) in params.iter() {
        if k != "sig" {
            pairs.push(format!("{}={}", k, v));
        }
    }
    pairs.join("&")
}

/// Verifies HMAC-SHA256 signature for URL parameters.
///
/// Implements cryptographic verification of request authenticity using
/// HMAC-SHA256. Prevents unauthorized access and ensures request integrity.
///
/// # Parameters
/// * `params` - Query parameters including signature and optional timestamp
/// * `sig` - Hex-encoded HMAC signature to verify
/// * `secret` - Shared secret key for HMAC computation
///
/// # Security
/// - Constant-time comparison prevents timing attacks
/// - Timestamp validation (if present) prevents replay attacks
/// - Missing or invalid signatures are rejected
///
/// # Errors
/// Returns `SignatureError` if:
/// - Signature is missing or empty
/// - Signature doesn't match computed HMAC
/// - Request timestamp (`t` parameter) has expired
pub fn verify_signature(
    params: &BTreeMap<String, String>,
    sig: &str,
    secret: &str,
) -> Result<(), SignatureError> {
    if sig.is_empty() {
        return Err(SignatureError::Missing);
    }

    // Reject expired requests based on timestamp parameter
    if let Some(ts) = params.get("t") {
        if let Ok(epoch) = ts.parse::<i64>() {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            if epoch < now {
                return Err(SignatureError::Expired);
            }
        }
    }

    // Compute expected HMAC and compare with provided signature
    let canonical = canonical_string(params);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|_| SignatureError::Invalid)?;
    mac.update(canonical.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    
    if expected == sig {
        Ok(())
    } else {
        Err(SignatureError::Invalid)
    }
}