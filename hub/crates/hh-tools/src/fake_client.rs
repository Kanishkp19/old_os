//! Scripted client implementing the API_SPEC §12 flow — used by CI
//! integration tests and the fault-injection harness.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;

#[derive(Subcommand)]
pub enum Action {
    /// Pair with a hub using a QR payload string.
    Pair {
        #[arg(long)]
        qr: String,
        #[arg(long, default_value = "fake-client")]
        name: String,
        /// Where to store the device identity (cert+key PEM bundle).
        #[arg(long, default_value = "fake-client.identity.pem")]
        identity: PathBuf,
    },
    /// Upload a file (chunked, verified, resumable).
    Upload {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value = "127.0.0.1:47800")]
        hub: String,
        #[arg(long, default_value = "fake-client.identity.pem")]
        identity: PathBuf,
    },
    /// Probe the UDP broadcast fallback discovery.
    Discover,
}

pub async fn run(action: Action) -> Result<()> {
    match action {
        Action::Pair { qr, name, identity } => pair(&qr, &name, &identity).await,
        Action::Upload { file, hub, identity } => upload_with_identity(&hub, &file, &identity).await,
        Action::Discover => discover().await,
    }
}

/// Parse `homehub://pair?h=<hub>&t=<token>&fp=<fp>&a=<ip:port,...>&n=<name>`.
pub fn parse_qr(payload: &str) -> Result<(String, String, String, Vec<String>, String)> {
    if payload.len() > 8192 { return Err(anyhow!("QR payload too large")); }
    let url = url::Url::parse(payload).context("invalid QR payload")?;
    if url.scheme() != "homehub" || url.host_str() != Some("pair") || !["", "/"].contains(&url.path()) {
        return Err(anyhow!("invalid pairing QR endpoint"));
    }
    let get = |k: &str| {
        url.query_pairs()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| anyhow!("missing {k}"))
    };
    let token = get("t")?;
    if token.len() != 22 || !token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        return Err(anyhow!("invalid pairing token"));
    }
    let fp = get("fp_sha256")?;
    if fp.len() != 64 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid Hub fingerprint"));
    }
    let addrs: Vec<String> = get("a")?.split(',').map(String::from).collect();
    if addrs.is_empty() || addrs.iter().any(|addr| {
        let Ok(parsed) = url::Url::parse(&format!("https://{addr}")) else { return true; };
        parsed.username() != "" || parsed.password().is_some() || parsed.path() != "/"
            || parsed.query().is_some() || parsed.fragment().is_some() || parsed.port() != Some(47802)
            || parsed.host_str().is_none()
    }) { return Err(anyhow!("invalid Hub address")); }
    Ok((get("h")?, token, fp, addrs, get("n")?))
}

async fn pair(qr: &str, name: &str, identity_path: &Path) -> Result<()> {
    let (_hub_id, token, fp, addrs, _hub_name) = parse_qr(qr)?;
    let addr = addrs.first().ok_or_else(|| anyhow!("no Hub address"))?;

    // Device keypair + CSR (keystore-backed on real clients).
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new())?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, name.to_string());
    let key = rcgen::KeyPair::generate()?;
    let csr_pem = params.serialize_request(&key)?.pem()?;

    // First, fetch the CA from the pairing server WITHOUT trusting it, then
    // verify its fingerprint matches the QR `fp` before sending the token
    // (T3: never send the token over unverified TLS).
    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .danger_accept_invalid_certs(true) // chain pinned manually below
        .danger_accept_invalid_hostnames(true)
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let response=client.get(format!("https://{addr}/pair/ca")).send().await?.error_for_status()?;
    if response.content_length().is_some_and(|n|n>65536){return Err(anyhow!("oversized CA response"));}
    let mut response = response;
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len() + chunk.len() > 65536 { return Err(anyhow!("oversized CA response")); }
        body.extend_from_slice(&chunk);
    }
    let ca_pem=serde_json::from_slice::<serde_json::Value>(&body)?["ca_cert_pem"].as_str().ok_or_else(||anyhow!("pairing CA missing"))?.to_owned();
    let verifier=hh_net::tls::PinnedCaVerifier::new(&ca_pem)?;
    if !verifier.fingerprint_matches(&fp){return Err(anyhow!("Hub does not match scanned QR"));}
    let mut tls=rustls::ClientConfig::builder().with_root_certificates(rustls::RootCertStore::empty()).with_no_client_auth();
    tls.dangerous().set_certificate_verifier(verifier);
    let client=reqwest::Client::builder().use_preconfigured_tls(tls)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20)).build()?;

    let resp = client
        .post(format!("https://{addr}/pair"))
        .json(&serde_json::json!({
            "token": token,
            "device_name": name,
            "platform": "linux",
            "model": "fake-client",
            "app_version": env!("CARGO_PKG_VERSION"),
            "csr_pem": csr_pem,
        }))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(anyhow!("pairing failed: {} {}", resp.status(), resp.text().await?));
    }
    let body: serde_json::Value = resp.json().await?;
    let cert_pem = body["cert_pem"].as_str().ok_or_else(|| anyhow!("no cert in response"))?;
    let ca_cert_pem = body["ca_cert_pem"].as_str().unwrap_or_default();

    // Identity bundle: device key + device cert + CA cert (client-side trust).
    let bundle = format!("{}{}\n{}", key.serialize_pem(), cert_pem, ca_cert_pem);
    use std::io::Write;
    let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let mut saved=options.open(identity_path)?;saved.write_all(bundle.as_bytes())?;saved.sync_all()?;
    println!("paired as {} (fp {}...) — identity saved to {}",
        body["device_id"].as_str().unwrap_or("?"), fp, identity_path.display());
    Ok(())
}

pub fn write_synthetic(path: &Path, bytes: u64) -> Result<()> {
    use std::io::Write;
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    let chunk = vec![0xCDu8; 1 << 20];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(chunk.len() as u64) as usize;
        f.write_all(&chunk[..n])?;
        left -= n as u64;
    }
    Ok(())
}

/// Full API_SPEC §12 flow: create → chunks (with X-Chunk-Hash) → complete.
pub async fn upload_file(hub: &str, path: &Path) -> Result<()> {
    upload_with_identity(hub, path, Path::new("fake-client.identity.pem")).await
}

async fn upload_with_identity(hub: &str, path: &Path, identity_path: &Path) -> Result<()> {
    use std::io::{Read, Seek, SeekFrom};
    let bundle = std::fs::read_to_string(identity_path)
        .context("read identity (run fake-client pair first)")?;
    let (identity, ca) = split_identity_bundle(&bundle);
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    let mut hash_buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut hash_buffer)?;
        if read == 0 { break; }
        hasher.update(&hash_buffer[..read]);
    }
    let root_hash = hasher.finalize().to_hex().to_string();
    let chunk_size = hh_core::DEFAULT_CHUNK_SIZE as usize;
    let name = path.file_name().unwrap().to_string_lossy().to_string();

    // The hub presents its own CA: the CA must be the trust anchor. Identity
    // alone provides client auth, not server trust (UnknownIssuer otherwise).
    let mut builder = reqwest::Client::builder()
        .use_rustls_tls()
        .identity(reqwest::Identity::from_pem(identity.as_bytes())?)
        .danger_accept_invalid_hostnames(true);
    if !ca.is_empty() {
        builder = builder.add_root_certificate(reqwest::Certificate::from_pem(ca.as_bytes())?);
    }
    let client = builder.build()?;

    let created: serde_json::Value = client
        .post(format!("https://{hub}/v1/transfers"))
        .json(&serde_json::json!({
            "name": name,
            "size": size,
            "mime": "application/octet-stream",
            "kind": "send",
            "chunk_size": chunk_size,
            "client_item_id": format!("fake:{}", name),
            "root_hash": root_hash,
        }))
        .send()
        .await?
        .json()
        .await?;
    if created["already_exists"].as_bool() == Some(true) {
        println!("already on hub (dedupe): {}", created["existing_file_id"]);
        return Ok(());
    }
    let transfer_id = created["transfer_id"].as_str().ok_or_else(|| anyhow!("no transfer_id"))?;
    let chunk_count = created["chunk_count"].as_u64().unwrap_or(0) as usize;
    let expected_chunks = size.div_ceil(chunk_size as u64).max(1) as usize;
    if chunk_count != expected_chunks {
        return Err(anyhow!("unexpected transfer chunk count"));
    }

    // Resume: skip chunks the hub already verified.
    let mut have = vec![false; chunk_count];
    for range in created["have"]["ranges"].as_array().cloned().unwrap_or_default() {
        let a = range[0].as_u64().unwrap_or(0) as usize;
        let b = range[1].as_u64().unwrap_or(0) as usize;
        for i in a..=b.min(chunk_count.saturating_sub(1)) {
            if let Some(present) = have.get_mut(i) { *present = true; }
        }
    }

    for idx in 0..chunk_count {
        if have[idx] {
            continue;
        }
        let start = idx as u64 * chunk_size as u64;
        let len = (size - start).min(chunk_size as u64) as usize;
        let mut chunk = vec![0u8; len];
        file.seek(SeekFrom::Start(start))?;
        file.read_exact(&mut chunk).context("source changed during upload")?;
        let hash = blake3::hash(&chunk).to_hex().to_string();
        let resp = client
            .put(format!("https://{hub}/v1/transfers/{transfer_id}/chunks/{idx}"))
            .header("content-type", "application/octet-stream")
            .header("x-chunk-hash", hash)
            .body(chunk.clone())
            .send()
            .await?;
        if resp.status().as_u16() == 409 {
            // Hash mismatch — retry the chunk (TR-06).
            let hash = blake3::hash(&chunk).to_hex().to_string();
            client
                .put(format!("https://{hub}/v1/transfers/{transfer_id}/chunks/{idx}"))
                .header("content-type", "application/octet-stream")
                .header("x-chunk-hash", hash)
                .body(chunk)
                .send()
                .await?
                .error_for_status()?;
        } else {
            resp.error_for_status()?;
        }
    }

    let done: serde_json::Value = client
        .post(format!("https://{hub}/v1/transfers/{transfer_id}/complete"))
        .json(&serde_json::json!({ "root_hash": root_hash }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    println!("uploaded ✓ verified: file_id={} size={}", done["file_id"], done["size"]);
    Ok(())
}

/// Split the PEM bundle written by `pair` ([key][device cert][CA cert]) into
/// (identity_pem, ca_pem). The identity keeps the key + leaf/chain certs; the
/// last certificate is the CA (trust anchor for `add_root_certificate`).
fn split_identity_bundle(bundle: &str) -> (String, String) {
    let mut key = String::new();
    let mut certs: Vec<String> = Vec::new();
    let mut block = String::new();
    let mut in_block = false;
    for line in bundle.lines() {
        if line.starts_with("-----BEGIN ") {
            in_block = true;
            block.clear();
        }
        if in_block {
            block.push_str(line);
            block.push('\n');
        }
        if line.starts_with("-----END ") {
            if block.contains("PRIVATE KEY") {
                key.push_str(&block);
            } else if block.contains("CERTIFICATE") {
                certs.push(std::mem::take(&mut block));
            }
            in_block = false;
        }
    }
    let mut identity = key;
    let ca = if certs.len() > 1 {
        certs.pop().unwrap_or_default()
    } else {
        String::new()
    };
    for c in &certs {
        identity.push_str(c);
    }
    (identity, ca)
}

async fn discover() -> Result<()> {
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await?;
    sock.set_broadcast(true)?;
    sock.send_to(hh_core::DISCOVERY_MAGIC, ("255.255.255.255", hh_core::DISCOVERY_PORT)).await?;
    let mut buf = [0u8; 1024];
    match tokio::time::timeout(std::time::Duration::from_secs(3), sock.recv_from(&mut buf)).await {
        Ok(Ok((n, from))) => {
            println!("hub at {from}: {}", String::from_utf8_lossy(&buf[..n]));
        }
        _ => println!("no hub answered the broadcast probe"),
    }
    Ok(())
}
