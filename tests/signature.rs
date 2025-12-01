use imagekit::signature::verify_signature;
use std::collections::BTreeMap;
use hmac::Mac;

#[test]
fn signature_validates() {
    let mut params = BTreeMap::new();
    params.insert("url".to_string(), "https://example.com/a.jpg".to_string());
    params.insert("w".to_string(), "400".to_string());
    let secret = "s";
    // compute expected
    let canonical = params.iter().map(|(k,v)| format!("{}={}",k,v)).collect::<Vec<_>>().join("&");
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(canonical.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    assert!(verify_signature(&params, &sig, secret).is_ok());
}

#[test]
fn signature_rejects_tamper() {
    let mut params = BTreeMap::new();
    params.insert("url".to_string(), "https://example.com/a.jpg".to_string());
    let secret = "s";
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(b"bad=param");
    let sig = hex::encode(mac.finalize().into_bytes());
    assert!(verify_signature(&params, &sig, secret).is_err());
}