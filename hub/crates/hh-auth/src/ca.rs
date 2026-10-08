//! Hub Certificate Authority (SECURITY §4).
//!
//! - ECDSA P-256 CA generated on first run.
//! - CA private key stored DPAPI-protected on Windows / mode 0600 elsewhere;
//!   never in hub.db plaintext (BACKEND_SCHEMA §1 `ca_key_ref`).
//! - Device certs: 1-year validity, CN = device_id, SAN URI
//!   `homehub:device:<id>`.

use std::path::Path;
use std::sync::Arc;

use hh_core::error::{Error, Result};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams,
    DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose, SanType,
};
use sha2::{Digest, Sha256};

/// One year in days, minus a small margin.
const DEVICE_CERT_DAYS: i64 = 365;
const CA_CERT_DAYS: i64 = 3650;

/// Hub CA + server cert bundle held in memory for the process lifetime.
pub struct HubIdentity {
    pub ca_cert_pem: String,
    pub server_cert_pem: String,
    ca_params: CertificateParams,
    /// DER of the CA cert exactly as stored on disk — the fingerprint must
    /// match what the client receives during pairing (T3), so it is computed
    /// from these bytes, never from a re-signed certificate.
    ca_cert_der: Vec<u8>,
    ca_key: KeyPair,
    server_key_pem: String,
}

impl HubIdentity {
    /// Load existing CA from disk, or create one on first run.
    pub fn load_or_create(data_dir: &Path, hub_name: &str) -> Result<Arc<Self>> {
        let ca_key_path = data_dir.join("ca.key.enc");
        let ca_cert_path = data_dir.join("ca.cert.pem");
        let server_key_path = data_dir.join("server.key.enc");
        let server_cert_path = data_dir.join("server.cert.pem");

        if ca_cert_path.exists() && ca_key_path.exists() {
            let ca_cert_pem = std::fs::read_to_string(&ca_cert_path)?;
            let ca_key_pem = read_protected_key(&ca_key_path)?;
            let ca_key = KeyPair::from_pem(&ca_key_pem)
                .map_err(|e| Error::Crypto(format!("CA key parse: {e}")))?;
            let ca_params = CertificateParams::from_ca_cert_pem(&ca_cert_pem)
                .map_err(|e| Error::Crypto(format!("CA cert parse: {e}")))?;
            let ca_cert_der = pem_body_der(&ca_cert_pem)?;
            let server_cert_pem = std::fs::read_to_string(&server_cert_path)?;
            let server_key_pem = read_protected_key(&server_key_path)?;
            Ok(Arc::new(Self {
                ca_cert_pem,
                server_cert_pem,
                ca_params,
                ca_cert_der,
                ca_key,
                server_key_pem,
            }))
        } else {
            let id = Self::generate(hub_name)?;
            write_protected_key(&ca_key_path, &id.ca_key.serialize_pem())?;
            std::fs::write(&ca_cert_path, &id.ca_cert_pem)?;
            write_protected_key(&server_key_path, &id.server_key_pem)?;
            std::fs::write(&server_cert_path, &id.server_cert_pem)?;
            Ok(Arc::new(id))
        }
    }

    fn generate(hub_name: &str) -> Result<Self> {
        // --- Hub CA ---
        let mut ca_params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| Error::Crypto(e.to_string()))?;
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("{hub_name} CA"));
        dn.push(DnType::OrganizationName, "Home Hub");
        ca_params.distinguished_name = dn;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let (not_before, not_after) = validity_window(CA_CERT_DAYS);
        ca_params.not_before = not_before;
        ca_params.not_after = not_after;
        let ca_key = KeyPair::generate().map_err(|e| Error::Crypto(e.to_string()))?;
        let ca_cert = ca_params
            .clone()
            .self_signed(&ca_key)
            .map_err(|e| Error::Crypto(e.to_string()))?;

        // --- Server cert (signed by CA) ---
        let server_key = KeyPair::generate().map_err(|e| Error::Crypto(e.to_string()))?;
        // The hub name is display text and may contain spaces/punctuation,
        // which are invalid in a DNS SAN (LibreSSL rejects the whole cert with
        // "unsupported or invalid name syntax"). Coerce it into a DNS label
        // and drop it entirely when nothing valid remains.
        let dns_name: String = hub_name
            .to_ascii_lowercase()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        let dns_name = dns_name.trim_matches('-');
        let mut san_dns = vec!["homehub.local".to_string()];
        if !dns_name.is_empty() {
            san_dns.push(dns_name.to_string());
        }
        let mut server_params = CertificateParams::new(san_dns)
            .map_err(|e| Error::Crypto(e.to_string()))?;
        // Loopback IP SANs: local tooling (curl, browsers on the same machine)
        // validates the hub by IP. LAN IPs are deliberately omitted — DHCP
        // volatility is covered by QR CA pinning (T3), not by cert SANs.
        server_params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::from([127, 0, 0, 1])));
        server_params
            .subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::from([
                0, 0, 0, 0, 0, 0, 0, 1,
            ])));
        let (not_before, not_after) = validity_window(DEVICE_CERT_DAYS * 5);
        server_params.not_before = not_before;
        server_params.not_after = not_after;
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .map_err(|e| Error::Crypto(e.to_string()))?;

        Ok(Self {
            ca_cert_pem: ca_cert.pem(),
            server_cert_pem: server_cert.pem(),
            ca_params,
            ca_cert_der: ca_cert.der().to_vec(),
            ca_key,
            server_key_pem: server_key.serialize_pem(),
        })
    }

    /// CA fingerprint for QR + mDNS TXT (TRD §4): first 16 hex chars of
    /// SHA-256 over the stored CA cert DER (stable across process restarts).
    pub fn fingerprint(&self) -> Result<String> {
        Ok(hub_fingerprint(&self.ca_cert_der))
    }

    pub fn server_key_pem(&self) -> &str {
        &self.server_key_pem
    }

    /// Issue a 1-year device certificate from a CSR (TRD §5 step 5).
    /// Returns (cert_pem, cert_serial_hex, expires_at_ms).
    pub fn issue_device_cert(&self, csr_pem: &str, device_id: &str) -> Result<(String, String, i64)> {
        let csr = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| Error::BadRequest(format!("invalid CSR: {e}")))?;

        // Rebuild params from the CSR with our identity fields and validity.
        let mut params = csr.params.clone();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, device_id.to_string());
        params.distinguished_name = dn;
        params.subject_alt_names = vec![SanType::URI(
            format!("homehub:device:{device_id}")
                .try_into()
                .map_err(|_| Error::Crypto("invalid SAN".into()))?,
        )];
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ClientAuth];
        let (not_before, not_after) = validity_window(DEVICE_CERT_DAYS);
        params.not_before = not_before;
        params.not_after = not_after;
        let serial = random_serial();
        params.serial_number = Some(rcgen::SerialNumber::from(serial.clone()));

        // Rebuild the issuer certificate from the stored CA params (same
        // subject + key, so chains verify against the on-disk CA cert).
        let ca_cert = self
            .ca_params
            .clone()
            .self_signed(&self.ca_key)
            .map_err(|e| Error::Crypto(e.to_string()))?;
        let cert = params
            .signed_by(&csr.public_key, &ca_cert, &self.ca_key)
            .map_err(|e| Error::Crypto(format!("sign device cert: {e}")))?;

        let serial_hex = serial.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let expires_at = not_after.unix_timestamp() * 1000;
        Ok((cert.pem(), serial_hex, expires_at))
    }

    /// Renew a device cert (<30 days to expiry, API_SPEC §4 `/certs/renew`).
    pub fn renew_device_cert(&self, csr_pem: &str, device_id: &str) -> Result<(String, String, i64)> {
        self.issue_device_cert(csr_pem, device_id)
    }
}

/// First 16 hex chars of SHA-256 over the CA cert DER (matches TRD §4 `fp=`).
pub fn hub_fingerprint(ca_cert_der: &[u8]) -> String {
    let digest = Sha256::digest(ca_cert_der);
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// DER bytes of the first PEM block (base64 body between the armor lines).
fn pem_body_der(pem: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    let body: String = pem
        .lines()
        .filter(|l| !l.starts_with("-----") && !l.trim().is_empty())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|e| Error::Crypto(format!("PEM body decode: {e}")))
}

fn validity_window(days: i64) -> (time::OffsetDateTime, time::OffsetDateTime) {
    let now = time::OffsetDateTime::now_utc();
    (
        now - time::Duration::days(1),
        now + time::Duration::days(days),
    )
}

fn random_serial() -> Vec<u8> {
    use rand::RngCore;
    let mut bytes = [0u8; 20];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes[0] = (bytes[0] & 0x7f) | 1; // positive with no leading zero
    bytes.to_vec()
}

// ---- protected key storage ----
// Windows: encrypt with DPAPI machine scope (CryptProtectData); the installer
// additionally ACLs the data dir to the service account. The DPAPI calls are
// isolated here so the Phase 4 Linux port swaps them for a mode-0600 file
// (already the unix behavior below).

#[cfg(windows)]
mod dpapi {
    //! Minimal, fully-typed FFI to CryptProtectData/CryptUnprotectData
    //! (crypt32.dll). Output is a CRYPT_INTEGER_BLOB whose pbData must be
    //! freed with LocalFree.

    use std::os::raw::{c_void, c_ulong};

    #[repr(C)]
    pub struct CryptBlob {
        pub cb_data: c_ulong,
        pub pb_data: *mut u8,
    }

    // DATA_BLOB fields map to the layout above (DWORD cbData; BYTE *pbData).
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            data_in: *const CryptBlob,
            sz_data_descr: *const u16,
            optional_entropy: *const CryptBlob,
            reserved: *mut c_void,
            prompt_struct: *mut c_void,
            flags: c_ulong,
            data_out: *mut CryptBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const CryptBlob,
            ppsz_data_descr: *mut *mut u16,
            optional_entropy: *const CryptBlob,
            reserved: *mut c_void,
            prompt_struct: *mut c_void,
            flags: c_ulong,
            data_out: *mut CryptBlob,
        ) -> i32;
        fn LocalFree(h: *mut c_void) -> *mut c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: c_ulong = 0x1;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn protect(plain: &[u8], description: &str) -> Result<Vec<u8>, String> {
        let input = CryptBlob {
            cb_data: plain.len() as c_ulong,
            pb_data: plain.as_ptr() as *mut u8,
        };
        let desc = wide(description);
        let mut out = CryptBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
        // SAFETY: all pointers are valid for the duration of the call; the
        // output blob is allocated by DPAPI and freed below with LocalFree.
        let ok = unsafe {
            CryptProtectData(
                &input,
                desc.as_ptr(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return Err("CryptProtectData failed".into());
        }
        let enc = unsafe { std::slice::from_raw_parts(out.pb_data, out.cb_data as usize) }.to_vec();
        unsafe { LocalFree(out.pb_data as *mut c_void) };
        Ok(enc)
    }

    pub fn unprotect(enc: &[u8]) -> Result<Vec<u8>, String> {
        let input = CryptBlob {
            cb_data: enc.len() as c_ulong,
            pb_data: enc.as_ptr() as *mut u8,
        };
        let mut out = CryptBlob { cb_data: 0, pb_data: std::ptr::null_mut() };
        let mut descr: *mut u16 = std::ptr::null_mut();
        // SAFETY: mirrors protect(); descr is allocated by DPAPI and freed.
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                &mut descr,
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut out,
            )
        };
        if ok == 0 {
            return Err("CryptUnprotectData failed".into());
        }
        let plain = unsafe { std::slice::from_raw_parts(out.pb_data, out.cb_data as usize) }.to_vec();
        unsafe {
            if !descr.is_null() {
                LocalFree(descr as *mut c_void);
            }
            LocalFree(out.pb_data as *mut c_void);
        }
        Ok(plain)
    }
}

// Magic header distinguishes DPAPI blobs from legacy plaintext PEM files so
// first migration from a pre-DPAPI build stays possible.
#[cfg(windows)]
const DPAPI_MAGIC: &[u8; 4] = b"DPA1";

#[cfg(windows)]
fn write_protected_key(path: &Path, pem: &str) -> Result<()> {
    let enc = dpapi::protect(pem.as_bytes(), "Home Hub CA key")
        .map_err(|e| Error::Crypto(e))?;
    let mut blob = DPAPI_MAGIC.to_vec();
    blob.extend_from_slice(&enc);
    std::fs::write(path, &blob)?;
    Ok(())
}

#[cfg(windows)]
fn read_protected_key(path: &Path) -> Result<String> {
    let raw = std::fs::read(path)?;
    let enc = match raw.strip_prefix(DPAPI_MAGIC.as_slice()) {
        // Current format: DPA1 || DPAPI blob.
        Some(rest) => rest.to_vec(),
        // Back-compat: a pre-DPAPI build wrote plaintext PEM (TODO removed in
        // this build). Re-protect it transparently on first read.
        None => {
            let pem = String::from_utf8(raw)
                .map_err(|_| Error::Crypto("key file is neither DPAPI blob nor UTF-8 PEM".into()))?;
            write_protected_key(path, &pem)?;
            return Ok(pem);
        }
    };
    let plain = dpapi::unprotect(&enc).map_err(Error::Crypto)?;
    String::from_utf8(plain).map_err(|_| Error::Crypto("decrypted key is not UTF-8".into()))
}

#[cfg(not(windows))]
fn write_protected_key(path: &Path, pem: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, pem)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(windows))]
fn read_protected_key(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)?)
}
