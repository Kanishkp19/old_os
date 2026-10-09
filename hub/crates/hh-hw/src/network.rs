//! Network inventory: interfaces, mode, link hints (API_SPEC §10).
//! Ethernet is preferred when present (FR-8.2); the hub records hub_network
//! rows with MACs so clients can send WoL packets (TRD §9).

use hh_core::time::now_ms;
use hh_core::Result;
use serde::Serialize;
use sysinfo::Networks;

use crate::{db_e, HwService};

#[derive(Debug, Clone, Serialize)]
pub struct NetworkInfo {
    pub interfaces: Vec<Iface>,
    pub mode: String, // ethernet | wifi | unknown
}

#[derive(Debug, Clone, Serialize)]
pub struct Iface {
    pub name: String,
    pub kind: String,
    pub ips: Vec<String>,
}

impl HwService {
    pub fn network_info(&self) -> Result<NetworkInfo> {
        let nets = Networks::new_with_refreshed_list();
        let mut ifaces = Vec::new();
        let mut has_eth = false;
        let mut has_wifi = false;
        for (name, data) in nets.iter() {
            let n = name.to_lowercase();
            let kind = if n.contains("eth") || n.contains("en0") || n.contains("ethernet") {
                "ethernet"
            } else if n.contains("wi") || n.contains("wlan") {
                "wifi"
            } else if n.contains("lo") {
                continue;
            } else {
                "unknown"
            };
            let ips: Vec<String> = data.ip_networks().iter().map(|i| i.addr.to_string()).collect();
            let active=ips.iter().filter_map(|ip|ip.parse::<std::net::IpAddr>().ok()).any(|ip|!ip.is_loopback()&&!ip.is_unspecified()&&!matches!(ip,std::net::IpAddr::V4(v4) if v4.is_link_local()));
            if active {if kind=="ethernet" {has_eth=true;} else if kind=="wifi" {has_wifi=true;}}
            ifaces.push(Iface { name: name.clone(), kind: kind.into(), ips });
        }
        let mode = if has_eth {
            "ethernet" // FR-8.2: Ethernet preferred
        } else if has_wifi {
            "wifi"
        } else {
            "unknown"
        }
        .to_string();
        Ok(NetworkInfo { interfaces: ifaces, mode })
    }

    /// Persist hub MAC addresses for client-side WoL (API_SPEC §8 wake-info).
    pub fn record_network(&self, macs: &[(String, String, String)]) -> Result<()> {
        let c = self.db.lock()?;
        for (iface, mac, kind) in macs {
            c.execute(
                "INSERT INTO hub_network (id, iface_name, mac_address, kind, updated_at)
                 VALUES (?1,?2,?3,?4,?5)
                 ON CONFLICT(id) DO UPDATE SET mac_address=excluded.mac_address, updated_at=excluded.updated_at",
                rusqlite::params![ulid::Ulid::new().to_string(), iface, mac, kind, now_ms()],
            )
            .map_err(db_e)?;
        }
        Ok(())
    }

    pub fn wake_info(&self) -> Result<Vec<(String, String)>> {
        let c = self.db.lock()?;
        let mut st = c
            .prepare("SELECT iface_name, mac_address FROM hub_network WHERE mac_address IS NOT NULL")
            .map_err(db_e)?;
        let rows = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(db_e)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_e)?;
        Ok(rows)
    }
}

/// Core status never probes public networks. Browser/update connectivity is
/// handled explicitly by those internet-enabled applications.
pub fn probe_internet() -> Option<bool> { None }
