//! State for the native quick settings drawer.
use hh_core::local_ipc::QuickSettingsUpdate;

#[derive(Debug, Clone)]
pub struct QuickSettingsState {
    pub is_open: bool,
    pub wifi_enabled: bool,
    pub wifi_ssid: Option<String>,
    pub volume_pct: u8,
    pub is_muted: bool,
    pub brightness_pct: u8,
    pub dnd_active: bool,
    pub is_pending: bool,
    pub error_message: Option<String>,
}

impl Default for QuickSettingsState {
    fn default() -> Self {
        Self { is_open: false, wifi_enabled: false, wifi_ssid: None, volume_pct: 50,
            is_muted: false, brightness_pct: 50, dnd_active: false,
            is_pending: false, error_message: None }
    }
}

impl QuickSettingsState {
    pub fn set_volume(&mut self, pct: u8) { self.volume_pct = pct.min(100); }
    pub fn toggle_mute(&mut self) { self.is_muted = !self.is_muted; }
    pub fn set_brightness(&mut self, pct: u8) { self.brightness_pct = pct.clamp(10, 100); }
    pub fn toggle_wifi(&mut self) { self.wifi_enabled = !self.wifi_enabled; }
    pub fn toggle_dnd(&mut self) { self.dnd_active = !self.dnd_active; }
    pub fn to_update(&self) -> QuickSettingsUpdate {
        QuickSettingsUpdate { wifi_enabled: Some(self.wifi_enabled), volume_pct: Some(self.volume_pct),
            is_muted: Some(self.is_muted), brightness_pct: Some(self.brightness_pct),
            dnd_active: Some(self.dnd_active) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brightness_clamps() {
        let mut state = QuickSettingsState::default();
        state.set_brightness(0);
        assert_eq!(state.brightness_pct, 10);
        state.set_brightness(255);
        assert_eq!(state.brightness_pct, 100);
    }
    #[test]
    fn volume_mute_and_payload() {
        let mut state = QuickSettingsState::default();
        state.set_volume(75);
        state.toggle_mute();
        state.toggle_wifi();
        assert_eq!(state.to_update().volume_pct, Some(75));
        assert_eq!(state.to_update().is_muted, Some(true));
        assert_eq!(state.to_update().wifi_enabled, Some(true));
    }
}
