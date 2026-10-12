//! Passphrase-encrypted identity bundles for an owner-approved migration.

use std::num::NonZeroU32;

use hh_core::error::{Error, Result};
use rand::RngCore;
use ring::{aead, pbkdf2};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 13] = b"HH_ID_MIG_V1\0";
const SALT_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const KDF_ITERATIONS: u32 = 100_000;
const MIN_BUNDLE_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN + 16;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityPayload {
    pub version: u32,
    pub hub_id: String,
    pub hub_name: String,
    pub ca_cert_pem: String,
    pub ca_key_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    pub devices: Vec<hh_db::DeviceRow>,
    pub created_at_ms: i64,
}

// A derived Debug implementation would print the CA and server private keys.
impl std::fmt::Debug for IdentityPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdentityPayload")
            .field("version", &self.version)
            .field("hub_id", &self.hub_id)
            .field("hub_name", &self.hub_name)
            .field("ca_cert_pem", &"[redacted]")
            .field("ca_key_pem", &"[redacted]")
            .field("server_cert_pem", &"[redacted]")
            .field("server_key_pem", &"[redacted]")
            .field("devices", &self.devices)
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

#[allow(clippy::too_many_arguments)] // Public migration API takes the complete identity explicitly.
pub fn export_identity_bundle(
    hub_id: &str,
    hub_name: &str,
    ca_cert_pem: &str,
    ca_key_pem: &str,
    server_cert_pem: &str,
    server_key_pem: &str,
    devices: &[hh_db::DeviceRow],
    passphrase: &str,
) -> Result<Vec<u8>> {
    if passphrase.len() < 8 {
        return Err(Error::Crypto(
            "Migration passphrase must be at least 8 bytes".into(),
        ));
    }
    validate_ca_pair(ca_cert_pem, ca_key_pem)?;
    let payload = IdentityPayload {
        version: 1,
        hub_id: hub_id.into(),
        hub_name: hub_name.into(),
        ca_cert_pem: ca_cert_pem.into(),
        ca_key_pem: ca_key_pem.into(),
        server_cert_pem: server_cert_pem.into(),
        server_key_pem: server_key_pem.into(),
        devices: devices.to_vec(),
        created_at_ms: hh_core::time::now_ms(),
    };
    seal_payload(&payload, passphrase)
}

pub fn import_identity_bundle(bundle_bytes: &[u8], passphrase: &str) -> Result<IdentityPayload> {
    if bundle_bytes.len() < MIN_BUNDLE_LEN || !bundle_bytes.starts_with(MAGIC) {
        return Err(Error::Crypto("Invalid migration bundle".into()));
    }
    let salt_start = MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let ciphertext_start = nonce_start + NONCE_LEN;
    let salt = &bundle_bytes[salt_start..nonce_start];
    let nonce_bytes: [u8; NONCE_LEN] = bundle_bytes[nonce_start..ciphertext_start]
        .try_into()
        .map_err(|_| Error::Crypto("Invalid migration bundle".into()))?;
    let key = encryption_key(salt, passphrase)?;
    let mut ciphertext = bundle_bytes[ciphertext_start..].to_vec();
    let plaintext = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce_bytes),
            aead::Aad::empty(),
            &mut ciphertext,
        )
        .map_err(|_| Error::Crypto("Invalid migration passphrase or corrupted bundle".into()))?;
    let payload: IdentityPayload = serde_json::from_slice(plaintext)
        .map_err(|_| Error::Crypto("Invalid migration payload".into()))?;
    if payload.version != 1 {
        return Err(Error::Crypto(
            "Unsupported migration payload version".into(),
        ));
    }
    validate_ca_pair(&payload.ca_cert_pem, &payload.ca_key_pem)?;
    Ok(payload)
}

fn validate_ca_pair(ca_cert_pem: &str, ca_key_pem: &str) -> Result<()> {
    let key = rcgen::KeyPair::from_pem(ca_key_pem)
        .map_err(|_| Error::Crypto("Invalid migration CA private key".into()))?;
    let (remaining, pem) = x509_parser::pem::parse_x509_pem(ca_cert_pem.as_bytes())
        .map_err(|_| Error::Crypto("Invalid migration CA certificate".into()))?;
    if !remaining.iter().all(u8::is_ascii_whitespace) || pem.label != "CERTIFICATE" {
        return Err(Error::Crypto("Invalid migration CA certificate".into()));
    }
    let (remaining_der, cert) = x509_parser::parse_x509_certificate(&pem.contents)
        .map_err(|_| Error::Crypto("Invalid migration CA certificate".into()))?;
    if !remaining_der.is_empty() || cert.public_key().raw != key.public_key_der() {
        return Err(Error::Crypto(
            "Migration CA key does not match certificate".into(),
        ));
    }
    Ok(())
}

fn encryption_key(salt: &[u8], passphrase: &str) -> Result<aead::LessSafeKey> {
    let mut key_bytes = [0u8; KEY_LEN];
    pbkdf2::derive(
        pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(KDF_ITERATIONS)
            .ok_or_else(|| Error::Crypto("Invalid KDF settings".into()))?,
        salt,
        passphrase.as_bytes(),
        &mut key_bytes,
    );
    let unbound = aead::UnboundKey::new(&aead::AES_256_GCM, &key_bytes)
        .map_err(|_| Error::Crypto("Migration encryption unavailable".into()))?;
    Ok(aead::LessSafeKey::new(unbound))
}

fn seal_payload(payload: &IdentityPayload, passphrase: &str) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    let mut rng = rand::rngs::OsRng;
    rng.fill_bytes(&mut salt);
    rng.fill_bytes(&mut nonce_bytes);
    let key = encryption_key(&salt, passphrase)?;
    let mut ciphertext = serde_json::to_vec(payload)
        .map_err(|_| Error::Crypto("Could not serialize migration payload".into()))?;
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce_bytes),
        aead::Aad::empty(),
        &mut ciphertext,
    )
    .map_err(|_| Error::Crypto("Could not encrypt migration payload".into()))?;
    let mut bundle = Vec::with_capacity(MIN_BUNDLE_LEN + ciphertext.len());
    bundle.extend_from_slice(MAGIC);
    bundle.extend_from_slice(&salt);
    bundle.extend_from_slice(&nonce_bytes);
    bundle.extend_from_slice(&ciphertext);
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cert_and_key() -> (String, String) {
        let key = rcgen::KeyPair::generate().unwrap();
        let params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        let cert = params.self_signed(&key).unwrap();
        (cert.pem(), key.serialize_pem())
    }

    fn bundle() -> Vec<u8> {
        let (cert, key) = cert_and_key();
        export_identity_bundle(
            "hub-1",
            "Home",
            &cert,
            &key,
            &cert,
            &key,
            &[],
            "correct horse",
        )
        .unwrap()
    }

    #[test]
    fn roundtrip() {
        let payload = import_identity_bundle(&bundle(), "correct horse").unwrap();
        assert_eq!(payload.version, 1);
        assert_eq!(payload.hub_id, "hub-1");
        assert!(!payload.ca_key_pem.is_empty());
        assert!(!format!("{payload:?}").contains(&payload.ca_key_pem));
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        assert!(matches!(
            import_identity_bundle(&bundle(), "wrong passphrase"),
            Err(Error::Crypto(message)) if message == "Invalid migration passphrase or corrupted bundle"
        ));
    }

    #[test]
    fn tampering_is_rejected() {
        let mut encrypted = bundle();
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        assert!(matches!(
            import_identity_bundle(&encrypted, "correct horse"),
            Err(Error::Crypto(_))
        ));
    }

    #[test]
    fn mismatched_ca_key_is_rejected_on_export_and_import() {
        let (cert, _) = cert_and_key();
        let (_, other_key) = cert_and_key();
        assert!(matches!(
            export_identity_bundle(
                "hub-1",
                "Home",
                &cert,
                &other_key,
                &cert,
                &other_key,
                &[],
                "correct horse"
            ),
            Err(Error::Crypto(_))
        ));
        let mut payload = import_identity_bundle(&bundle(), "correct horse").unwrap();
        payload.ca_key_pem = other_key;
        let encrypted = seal_payload(&payload, "correct horse").unwrap();
        assert!(matches!(
            import_identity_bundle(&encrypted, "correct horse"),
            Err(Error::Crypto(_))
        ));
    }
}
