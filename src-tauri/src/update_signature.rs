use ed25519_dalek::{Signature, VerifyingKey};
use serde_json::Value;
fn hex<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N * 2 {
        return Err("updates_signature_failed".into());
    }
    let mut bytes = [0u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair).map_err(|_| "updates_signature_failed")?;
        bytes[index] = u8::from_str_radix(text, 16).map_err(|_| "updates_signature_failed")?;
    }
    Ok(bytes)
}
pub fn message(version: &str, hash: &str, size: u64) -> String {
    format!("Questro update v1\n{version}\n{hash}\n{size}\n")
}
fn verify_with_key(
    key: &[u8; 32],
    document: &[u8],
    version: &str,
    hash: &str,
    size: u64,
) -> Result<(), String> {
    if document.len() > 4096 {
        return Err("updates_signature_failed".into());
    }
    let data: Value = serde_json::from_slice(document).map_err(|_| "updates_signature_failed")?;
    if data["algorithm"] != "Ed25519"
        || data["version"] != version
        || data["sha256"] != hash
        || data["size"] != size
    {
        return Err("updates_signature_failed".into());
    }
    let signature = Signature::from_bytes(&hex::<64>(data["signature"].as_str().unwrap_or(""))?);
    VerifyingKey::from_bytes(key)
        .map_err(|_| "updates_signature_failed")?
        .verify_strict(message(version, hash, size).as_bytes(), &signature)
        .map_err(|_| "updates_signature_failed".into())
}
pub fn verify(document: &[u8], version: &str, hash: &str, size: u64) -> Result<(), String> {
    verify_with_key(
        &hex::<32>(include_str!("../update-public-key.hex").trim())?,
        document,
        version,
        hash,
        size,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;
    #[test]
    fn signatures_bind_publisher_version_hash_and_size() {
        let key = SigningKey::from_bytes(&[42; 32]);
        let hash = "a".repeat(64);
        let version = "3.6.8";
        let size = 4096;
        let signature = key.sign(message(version, &hash, size).as_bytes());
        let signature: String = signature
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let document = json!({"algorithm":"Ed25519","version":version,"sha256":hash,"size":size,"signature":signature});
        let bytes = serde_json::to_vec(&document).unwrap();
        assert!(verify_with_key(
            &key.verifying_key().to_bytes(),
            &bytes,
            version,
            &hash,
            size
        )
        .is_ok());
        assert!(verify_with_key(
            &key.verifying_key().to_bytes(),
            &bytes,
            "9.9.9",
            &hash,
            size
        )
        .is_err());
        assert!(verify_with_key(
            &key.verifying_key().to_bytes(),
            &bytes,
            version,
            &"b".repeat(64),
            size
        )
        .is_err());
        assert!(verify_with_key(
            &key.verifying_key().to_bytes(),
            &bytes,
            version,
            &hash,
            size + 1
        )
        .is_err());
        assert!(verify_with_key(
            &SigningKey::from_bytes(&[43; 32]).verifying_key().to_bytes(),
            &bytes,
            version,
            &hash,
            size
        )
        .is_err());
        let mut damaged = document;
        damaged["signature"] = json!("00".repeat(64));
        assert!(verify_with_key(
            &key.verifying_key().to_bytes(),
            &serde_json::to_vec(&damaged).unwrap(),
            version,
            &hash,
            size
        )
        .is_err());
    }
}
