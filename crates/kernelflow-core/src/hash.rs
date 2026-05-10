//! Deterministic SHA-256 helpers used for content-addressed workflow IDs
//! and onchain attestation hashes.

use sha2::{Digest, Sha256};

/// Returns the lowercase hex SHA-256 of the canonical-JSON serialization
/// of `value`. Determinism is required so that distributed nodes produce
/// identical attestations.
pub fn hash_json<T: serde::Serialize>(value: &T) -> String {
    // Canonical JSON: sort keys via `serde_json::to_value` -> BTreeMap walk.
    let v = serde_json::to_value(value).expect("serialize");
    let canonical = canonicalize(&v);
    let bytes = serde_json::to_vec(&canonical).expect("serialize");
    let digest = Sha256::digest(&bytes);
    hex::encode(digest)
}

fn canonicalize(v: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let mut sorted: std::collections::BTreeMap<std::string::String, serde_json::Value> =
                std::collections::BTreeMap::new();
            for (k, val) in map {
                sorted.insert(k.clone(), canonicalize(val));
            }
            serde_json::to_value(sorted).unwrap()
        }
        Value::Array(a) => Value::Array(a.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn determinism() {
        let a = serde_json::json!({ "b": 1, "a": 2 });
        let b = serde_json::json!({ "a": 2, "b": 1 });
        assert_eq!(hash_json(&a), hash_json(&b));
    }
}
