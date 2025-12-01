use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("missing signature")] Missing,
    #[error("invalid signature")] Invalid,
    #[error("expired")] Expired,
}

// Compute canonical string over sorted parameters excluding `sig`
fn canonical_string(params: &BTreeMap<String, String>) -> String {
    let mut pairs: Vec<String> = Vec::new();
    for (k, v) in params.iter() {
        if k != "sig" { pairs.push(format!("{}={}", k, v)); }
    }
    pairs.join("&")
}

pub fn verify_signature(
    params: &BTreeMap<String, String>,
    sig: &str,
    secret: &str,
) -> Result<(), SignatureError> {
    if sig.is_empty() { return Err(SignatureError::Missing); }

    // Check expiry if present
    if let Some(ts) = params.get("t") {
        if let Ok(epoch) = ts.parse::<i64>() {
            let now = time::OffsetDateTime::now_utc().unix_timestamp();
            if epoch < now { return Err(SignatureError::Expired); }
        }
    }

    let canonical = canonical_string(params);
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| SignatureError::Invalid)?;
    mac.update(canonical.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if expected == sig { Ok(()) } else { Err(SignatureError::Invalid) }
}