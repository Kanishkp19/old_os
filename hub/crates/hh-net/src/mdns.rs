//! Discovery: mDNS advertisement of `_homehub._tcp` (TRD §4) plus the UDP
//! broadcast fallback responder on 47803 (API_SPEC §2).

use hh_core::error::Result;
use hh_core::{API_PORT, DISCOVERY_MAGIC, DISCOVERY_PORT, MDNS_SERVICE_TYPE, PAIRING_PORT};
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde_json::json;

/// Advertise the hub via mDNS. Returns a guard; drop to unadvertise.
pub struct MdnsAdvertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl MdnsAdvertisement {
    pub fn start(hub_id: &str, name: &str, ca_fp: &str, pairing_open: bool) -> Result<Self> {
        let daemon = ServiceDaemon::new().map_err(|e| hh_core::Error::Internal(format!("mdns: {e}")))?;
        let host = format!("{}.local.", hostname::short());
        let props = [
            ("id", hub_id),
            ("v", "1"),
            ("name", name),
            ("fp", ca_fp),
            ("pair", if pairing_open { "1" } else { "0" }),
        ];
        let info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            name,
            &host,
            "",
            API_PORT,
            &props[..],
        )
        .map_err(|e| hh_core::Error::Internal(format!("mdns service: {e}")))?
        .enable_addr_auto();
        let fullname = info.get_fullname().to_string();
        daemon
            .register(info)
            .map_err(|e| hh_core::Error::Internal(format!("mdns register: {e}")))?;
        Ok(Self { daemon, fullname })
    }

    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for MdnsAdvertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// UDP broadcast fallback responder (NW-02). Listens for `HHDISCOVER1` and
/// replies with the JSON descriptor.
pub async fn broadcast_responder(
    hub_id: String,
    name: String,
    fp: String,
) -> Result<()> {
    let sock = tokio::net::UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).await?;
    let mut buf = [0u8; 256];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((n, peer)) if &buf[..n] == DISCOVERY_MAGIC => {
                let reply = json!({
                    "id": hub_id,
                    "name": name,
                    "port": API_PORT,
                    "pair_port": PAIRING_PORT,
                    "fp": fp,
                });
                let _ = sock.send_to(reply.to_string().as_bytes(), peer).await;
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(error = %e, "discovery responder recv error");
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

/// Minimal hostname helper to avoid an extra dependency.
mod hostname {
    pub fn short() -> String {
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "homehub".into())
    }
}
