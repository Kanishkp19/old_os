//! TLS 1.3 server configuration and the mTLS client-cert verifier
//! (AGENTS.md §6: custom verifier checks chain to Hub CA AND consults the
//! in-memory revocation set per handshake).

use std::sync::Arc;

use hh_auth::{HubIdentity, RevocationList};
use hh_core::error::{Error, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::server::WebPkiClientVerifier;
use rustls::{DigitallySignedStruct, DistinguishedName, RootCertStore, SignatureScheme};

/// Identity extracted from the peer's device certificate at handshake time.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    pub device_id: String,
    pub cert_serial: String,
    pub scopes: Vec<String>,
}

impl PeerIdentity {
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

// ---- server side ----

/// Custom verifier: chain-verify against the Hub CA via webpki, then check
/// the revocation set (fail closed).
pub struct DeviceCertVerifier {
    inner: Arc<dyn ClientCertVerifier>,
    revocation: RevocationList,
}

// Manual impl: RevocationList is not Debug.
impl std::fmt::Debug for DeviceCertVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeviceCertVerifier")
            .field("inner", &"WebPkiClientVerifier")
            .finish_non_exhaustive()
    }
}

impl DeviceCertVerifier {
    pub fn new(ca: &HubIdentity, revocation: RevocationList) -> Result<Arc<Self>> {
        let mut roots = RootCertStore::empty();
        let ca_der = pem_first(&ca.ca_cert_pem)?;
        roots
            .add(CertificateDer::from(ca_der))
            .map_err(|e| Error::Crypto(format!("add CA to roots: {e}")))?;
        let inner = WebPkiClientVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|e| Error::Crypto(format!("client verifier: {e}")))?;
        Ok(Arc::new(Self { inner, revocation }))
    }
}

impl ClientCertVerifier for DeviceCertVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> std::result::Result<ClientCertVerified, rustls::Error> {
        self.inner.verify_client_cert(end_entity, intermediates, now)?;
        let (_, cert) = x509_parser::parse_x509_certificate(end_entity.as_ref())
            .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding))?;
        let serial = serial_hex(&cert);
        if self.revocation.is_revoked(&serial) {
            return Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::Revoked,
            ));
        }
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// TLS 1.3-only server config requiring device certs (port 47800).
pub fn server_config_mtls(ca: &HubIdentity, revocation: RevocationList) -> Result<Arc<rustls::ServerConfig>> {
    let certs = vec![CertificateDer::from(pem_first(&ca.server_cert_pem)?), CertificateDer::from(pem_first(&ca.ca_cert_pem)?)];
    let key = PrivateKeyDer::Pkcs8(pem_key(&ca.server_key_pem())?);
    let verifier = DeviceCertVerifier::new(ca, revocation)?;
    let provider = rustls::crypto::ring::default_provider();
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Error::Crypto(format!("tls versions: {e}")))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)
        .map_err(|e| Error::Crypto(format!("server cert: {e}")))?;
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

/// TLS 1.3 server config without client certs (pairing port 47802).
pub fn server_config_pairing(ca: &HubIdentity) -> Result<Arc<rustls::ServerConfig>> {
    let certs = vec![CertificateDer::from(pem_first(&ca.server_cert_pem)?), CertificateDer::from(pem_first(&ca.ca_cert_pem)?)];
    let key = PrivateKeyDer::Pkcs8(pem_key(&ca.server_key_pem())?);
    let provider = rustls::crypto::ring::default_provider();
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| Error::Crypto(format!("tls versions: {e}")))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| Error::Crypto(format!("server cert: {e}")))?;
    cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

// ---- client side (hh-tools fake-client: pins the CA fingerprint, T3) ----

/// Client verifier that accepts only the Hub CA whose SHA-256 fingerprint
/// starts with the 16-hex-char `fp` from the QR payload (API_SPEC §3).
#[derive(Debug)]
pub struct PinnedCaVerifier {
    ca_der: Vec<u8>,
}

impl PinnedCaVerifier {
    pub fn new(ca_cert_pem: &str) -> Result<Arc<Self>> {
        Ok(Arc::new(Self { ca_der: pem_first(ca_cert_pem)? }))
    }

    pub fn fingerprint_matches(&self, fingerprint: &str) -> bool {
        if fingerprint.len() == 16 {
            return hh_auth::hub_fingerprint(&self.ca_der).eq_ignore_ascii_case(fingerprint);
        }
        if fingerprint.len() != 64 || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        use sha2::Digest;
        let actual = sha2::Sha256::digest(&self.ca_der);
        actual.iter().enumerate().all(|(i, byte)| {
            u8::from_str_radix(&fingerprint[i * 2..i * 2 + 2], 16).ok() == Some(*byte)
        })
    }
}

impl ServerCertVerifier for PinnedCaVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, rustls::Error> {
        let mut roots = RootCertStore::empty();
        roots.add(CertificateDer::from(self.ca_der.clone()))
            .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadEncoding))?;
        let parsed = rustls::server::ParsedCertificate::try_from(end_entity)?;
        let provider = rustls::crypto::ring::default_provider();
        rustls::client::verify_server_cert_signed_by_trust_anchor(&parsed, &roots, _intermediates, _now,
            provider.signature_verification_algorithms.all)?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _m: &[u8],
        _c: &CertificateDer<'_>,
        _d: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(_m, _c, _d, &rustls::crypto::ring::default_provider().signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        _m: &[u8],
        _c: &CertificateDer<'_>,
        _d: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(_m, _c, _d, &rustls::crypto::ring::default_provider().signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PSS_SHA256,
        ]
    }
}

// ---- helpers ----

/// Parse a peer cert DER into (device_id from CN, serial hex).
pub fn parse_peer_cert(der: &[u8]) -> Result<(String, String)> {
    let (_, cert) = x509_parser::parse_x509_certificate(der)
        .map_err(|e| Error::Crypto(format!("peer cert parse: {e}")))?;
    let cn = cert
        .subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .unwrap_or("")
        .to_string();
    Ok((cn, serial_hex(&cert)))
}

pub fn serial_hex(cert: &x509_parser::certificate::X509Certificate<'_>) -> String {
    cert.serial.to_bytes_be().iter().map(|b| format!("{b:02x}")).collect()
}

fn pem_first(pem: &str) -> Result<Vec<u8>> {
    let mut rd = std::io::BufReader::new(pem.as_bytes());
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut rd)
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| Error::Crypto(format!("pem parse: {e}")))?;
    certs.into_iter().next().map(|c| c.as_ref().to_vec())
        .ok_or_else(|| Error::Crypto("no certificate in pem".into()))
}

fn pem_key(pem: &str) -> Result<rustls::pki_types::PrivatePkcs8KeyDer<'static>> {
    let mut rd = std::io::BufReader::new(pem.as_bytes());
    let mut it = rustls_pemfile::pkcs8_private_keys(&mut rd);
    let key = it
        .next()
        .transpose()
        .map_err(|e| Error::Crypto(format!("key parse: {e}")))?
        .ok_or_else(|| Error::Crypto("no pkcs8 key in pem".into()));
    drop(it);
    key
}

#[cfg(test)]
mod pairing_pin_tests {
    use super::*;

    #[test]
    fn rogue_hub_fingerprint_is_rejected() {
        let root = std::env::temp_dir().join(format!("hh-pin-{}", ulid::Ulid::new()));
        let trusted_dir = root.join("trusted");
        let rogue_dir = root.join("rogue");
        std::fs::create_dir_all(&trusted_dir).unwrap();
        std::fs::create_dir_all(&rogue_dir).unwrap();
        let trusted = HubIdentity::load_or_create(&trusted_dir, "Trusted Home").unwrap();
        let rogue = HubIdentity::load_or_create(&rogue_dir, "Rogue Home").unwrap();
        let scanned_fp = trusted.fingerprint().unwrap();
        let trusted_verifier = PinnedCaVerifier::new(&trusted.ca_cert_pem).unwrap();
        let rogue_verifier = PinnedCaVerifier::new(&rogue.ca_cert_pem).unwrap();
        std::fs::remove_dir_all(&root).unwrap();

        assert!(trusted_verifier.fingerprint_matches(&scanned_fp));
        assert!(!rogue_verifier.fingerprint_matches(&scanned_fp));
    }
}
