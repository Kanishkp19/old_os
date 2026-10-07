//! Auto-update check (W2.6, FR-1.6): the hub fetches a signed manifest and
//! verifies it against a pinned ed25519 public key before comparing versions.
//!
//! Manifest format (published by `hh-tools sign-manifest`):
//!
//! ```json
//! { "version": "0.2.0", "url": "https://…/HomeHubSetup.exe",
//!   "sha256": "<hex of the installer file>",
//!   "sig": "<hex ed25519 signature over \"version\\nurl\\nsha256\">" }
//! ```
//!
//! Configuration lives in the `settings` table so ops can change it without a
//! rebuild: `update.manifest_url` and `update.pubkey_hex`. Absent keys mean
//! the channel is simply disabled (the endpoint answers honestly).

use hh_core::error::{Error, Result};
use hh_db::Db;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateCheck {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub url: Option<String>,
    pub sha256: Option<String>,
    pub reason: Option<String>,
}

/// Canonical message that is signed: `"{version}\n{url}\n{sha256}"`.
pub fn signing_message(version: &str, url: &str, sha256: &str) -> Vec<u8> {
    format!("{version}\n{url}\n{sha256}").into_bytes()
}

/// Verify a manifest's ed25519 signature against the pinned key.
pub fn verify_manifest(raw: &str, pubkey_hex: &str) -> Result<(String, String, String)> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| Error::BadRequest(format!("manifest parse: {e}")))?;
    let version = v["version"].as_str().ok_or_else(|| Error::BadRequest("manifest: no version".into()))?;
    let url = v["url"].as_str().ok_or_else(|| Error::BadRequest("manifest: no url".into()))?;
    let sha256 = v["sha256"].as_str().ok_or_else(|| Error::BadRequest("manifest: no sha256".into()))?;
    let sig_hex = v["sig"].as_str().ok_or_else(|| Error::BadRequest("manifest: no sig".into()))?;

    let key_hex = pubkey_hex.trim();
    if key_hex.len() != 64 {
        return Err(Error::BadRequest("update pubkey must be 32 bytes hex".into()));
    }
    let key_bytes = hex_decode(key_hex).ok_or_else(|| Error::BadRequest("update pubkey not hex".into()))?;
    let vk = VerifyingKey::from_bytes(&key_bytes)
        .map_err(|e| Error::BadRequest(format!("update pubkey invalid: {e}")))?;
    let sig_bytes = hex_decode(sig_hex).ok_or_else(|| Error::BadRequest("manifest sig not hex".into()))?;
    let sig_arr: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| Error::BadRequest("manifest sig must be 64 bytes".into()))?;
    let sig = Signature::from_bytes(&sig_arr);
    vk.verify(&signing_message(version, url, sha256), &sig)
        .map_err(|_| Error::BadRequest("manifest signature invalid".into()))?;
    Ok((version.to_string(), url.to_string(), sha256.to_string()))
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

/// Fetch + verify the manifest from the configured URL. Blocking network IO —
/// call from a spawn_blocking context.
pub fn fetch_and_verify(manifest_url: &str, pubkey_hex: &str) -> Result<(String, String, String)> {
    let resp = ureq::get(manifest_url)
        .timeout(std::time::Duration::from_secs(10))
        .call()
        .map_err(|e| Error::Internal(format!("manifest fetch: {e}")))?;
    let mut resp = resp;
    let body = resp
        .into_string()
        .map_err(|e| Error::Internal(format!("manifest read: {e}")))?;
    verify_manifest(&body, pubkey_hex)
}

/// Compare dotted numeric versions: true when `latest > current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }
    let (a, b) = (parts(latest), parts(current));
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Read channel settings and run the check (blocking; use spawn_blocking).
pub fn check_from_settings(db: &Db, current_version: &str) -> UpdateCheck {
    let base = UpdateCheck {
        available: false,
        current_version: current_version.to_string(),
        latest_version: None,
        url: None,
        sha256: None,
        reason: None,
    };
    let url = match db.get_setting("update.manifest_url") {
        Ok(Some(u)) if !u.is_empty() => u,
        _ => return UpdateCheck { reason: Some("channel_not_configured".into()), ..base },
    };
    let key = match db.get_setting("update.pubkey_hex") {
        Ok(Some(k)) if !k.is_empty() => k,
        _ => return UpdateCheck { reason: Some("channel_not_configured".into()), ..base },
    };
    match fetch_and_verify(&url, &key) {
        Ok((version, download, sha256)) => {
            let available = is_newer(&version, current_version);
            UpdateCheck {
                available,
                latest_version: Some(version),
                url: Some(download),
                sha256: Some(sha256),
                reason: None,
                ..base
            }
        }
        Err(e) => UpdateCheck { reason: Some(format!("{e}")), ..base },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.1"));
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        use ed25519_dalek::{Signer, SigningKey};
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = hex_encode(&sk.verifying_key().to_bytes());
        let (version, url, sha) = ("0.2.0", "https://example.com/a.exe", "deadbeef");
        let sig = hex_encode(&sk.sign(&signing_message(version, url, sha)).to_bytes());
        let manifest = serde_json::json!({ "version": version, "url": url, "sha256": sha, "sig": sig });
        let out = verify_manifest(&manifest.to_string(), &pk).expect("verify");
        assert_eq!(out.0, version);
        // Tampering breaks it.
        let mut bad = manifest.clone();
        bad["version"] = serde_json::json!("9.9.9");
        assert!(verify_manifest(&bad.to_string(), &pk).is_err());
    }

    fn hex_encode(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
}
