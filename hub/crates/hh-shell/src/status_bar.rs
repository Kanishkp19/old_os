//! Status bar state derived from the owner-only shell snapshot.
use hh_core::local_ipc::ShellSnapshot;

pub const HEIGHT: u16 = crate::theme::GEOMETRY_STATUS_BAR_HEIGHT;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusBarState {
    pub hub_name: String,
    pub clock_text: String,
    pub connected_devices: u32,
    pub storage_healthy: bool,
    pub storage_available_bytes: u64,
    pub wifi_ssid: Option<String>,
    pub volume_pct: u8,
    pub is_muted: bool,
    pub battery_pct: Option<u8>,
    pub is_charging: bool,
    pub notification_count: u32,
    pub dnd_active: bool,
}

impl Default for StatusBarState {
    fn default() -> Self {
        Self {
            hub_name: "Home Computer".into(),
            clock_text: String::new(),
            connected_devices: 0,
            storage_healthy: true,
            storage_available_bytes: 0,
            wifi_ssid: None,
            volume_pct: 0,
            is_muted: false,
            battery_pct: None,
            is_charging: false,
            notification_count: 0,
            dnd_active: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndicatorItem {
    pub id: &'static str,
    pub label: String,
    pub accessible_name: String,
    pub icon_glyph: &'static str,
    pub warning: bool,
}

impl StatusBarState {
    /// Snapshot does not carry mute, notifications or DND; those are local UI state.
    pub fn update_from_snapshot(&mut self, snapshot: &ShellSnapshot) {
        let minutes = snapshot.timestamp.div_euclid(60);
        let clock_text = format!(
            "{:02}:{:02} UTC",
            minutes.div_euclid(60).rem_euclid(24),
            minutes.rem_euclid(60)
        );
        *self = Self {
            hub_name: snapshot.hub_name.clone(),
            clock_text,
            connected_devices: snapshot.connected_devices,
            storage_healthy: snapshot.storage_healthy,
            storage_available_bytes: snapshot.storage_available_bytes,
            wifi_ssid: snapshot.wifi_ssid.clone(),
            volume_pct: snapshot.volume_pct,
            battery_pct: snapshot.battery_pct,
            is_charging: snapshot.is_charging,
            is_muted: self.is_muted,
            notification_count: self.notification_count,
            dnd_active: self.dnd_active,
        };
    }

    pub fn indicators(&self) -> Vec<IndicatorItem> {
        let devices = format!(
            "{} {} connected",
            self.connected_devices,
            if self.connected_devices == 1 {
                "device"
            } else {
                "devices"
            }
        );
        let storage = if self.storage_healthy {
            format!(
                "Storage healthy: {} bytes available",
                self.storage_available_bytes
            )
        } else {
            "Storage Warning: Check disk".into()
        };
        let mut items = vec![
            IndicatorItem {
                id: "devices",
                label: devices.clone(),
                accessible_name: devices,
                icon_glyph: "devices",
                warning: false,
            },
            IndicatorItem {
                id: "storage",
                label: storage.clone(),
                accessible_name: storage,
                icon_glyph: "storage",
                warning: !self.storage_healthy,
            },
        ];
        if let Some(ssid) = &self.wifi_ssid {
            items.push(IndicatorItem {
                id: "wifi",
                label: ssid.clone(),
                accessible_name: format!("Wi-Fi connected to {ssid}"),
                icon_glyph: "wifi",
                warning: false,
            });
        }
        let volume = if self.is_muted {
            "Muted".into()
        } else {
            format!("Volume {}%", self.volume_pct)
        };
        items.push(IndicatorItem {
            id: "volume",
            label: volume.clone(),
            accessible_name: volume,
            icon_glyph: "volume",
            warning: false,
        });
        if let Some(pct) = self.battery_pct {
            let label = format!(
                "Battery {pct}%{}",
                if self.is_charging { ", charging" } else { "" }
            );
            items.push(IndicatorItem {
                id: "battery",
                label: label.clone(),
                accessible_name: label,
                icon_glyph: "battery",
                warning: pct <= 20 && !self.is_charging,
            });
        }
        items
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> ShellSnapshot {
        ShellSnapshot {
            timestamp: 0,
            hub_name: "Home Computer".into(),
            storage_available_bytes: 42,
            storage_healthy: true,
            connected_devices: 2,
            wifi_ssid: Some("Hub".into()),
            volume_pct: 50,
            battery_pct: Some(85),
            is_charging: false,
            is_locked: false,
        }
    }
    #[test]
    fn battery_is_only_shown_when_reported() {
        let mut state = StatusBarState::default();
        state.update_from_snapshot(&snapshot());
        assert!(state.indicators().iter().any(|item| item.id == "battery"));
        let mut desktop = snapshot();
        desktop.battery_pct = None;
        state.update_from_snapshot(&desktop);
        assert!(!state.indicators().iter().any(|item| item.id == "battery"));
    }
    #[test]
    fn storage_warning_and_snapshot_fields() {
        let mut state = StatusBarState::default();
        let mut data = snapshot();
        data.storage_healthy = false;
        state.update_from_snapshot(&data);
        assert_eq!(state.connected_devices, 2);
        assert_eq!(state.clock_text, "00:00 UTC");
        assert_eq!(state.wifi_ssid.as_deref(), Some("Hub"));
        let storage = state
            .indicators()
            .into_iter()
            .find(|item| item.id == "storage")
            .unwrap();
        assert!(storage.warning);
        assert_eq!(storage.label, "Storage Warning: Check disk");
    }
}
