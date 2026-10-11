//! Resumable first-boot setup state. Credentials are never stored in the checkpoint.
use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OobeStep { Language, InputMode, Network, OwnerSecurity, DevicePairing, Complete }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OobeState {
    pub current_step: OobeStep,
    pub selected_language: String,
    pub selected_input_mode: String,
    pub network_configured: bool,
    pub owner_configured: bool,
    pub completed: bool,
    pub pairing_qr_ready: bool,
}

impl Default for OobeState {
    fn default() -> Self {
        Self { current_step: OobeStep::Language, selected_language: "en".into(),
            selected_input_mode: "mouse".into(), network_configured: false,
            owner_configured: false, completed: false, pairing_qr_ready: false }
    }
}

impl OobeState {
    pub fn select_language(&mut self, language: &str) -> bool {
        if !matches!(language, "en" | "hi") { return false; }
        self.selected_language = language.into();
        true
    }
    pub fn select_input_mode(&mut self, mode: &str) -> bool {
        if !matches!(mode, "touch" | "mouse") { return false; }
        self.selected_input_mode = mode.into();
        true
    }
    pub fn configure_owner(&mut self, secret: &str) -> bool {
        if secret.chars().count() < 4 { return false; }
        self.owner_configured = true;
        true
    }
    pub fn advance_step(&mut self) {
        self.current_step = match self.current_step {
            OobeStep::Language => OobeStep::InputMode,
            OobeStep::InputMode => OobeStep::Network,
            OobeStep::Network => OobeStep::OwnerSecurity,
            OobeStep::OwnerSecurity if self.owner_configured => OobeStep::DevicePairing,
            OobeStep::DevicePairing => OobeStep::Complete,
            step => step,
        };
        if self.current_step == OobeStep::DevicePairing { self.pairing_qr_ready = true; }
        self.completed = self.current_step == OobeStep::Complete;
    }
    pub fn go_back(&mut self) {
        self.current_step = match self.current_step {
            OobeStep::Language => OobeStep::Language,
            OobeStep::InputMode => OobeStep::Language,
            OobeStep::Network => OobeStep::InputMode,
            OobeStep::OwnerSecurity => OobeStep::Network,
            OobeStep::DevicePairing => OobeStep::OwnerSecurity,
            OobeStep::Complete => OobeStep::DevicePairing,
        };
        self.completed = false;
    }
    pub fn save_checkpoint(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        let temporary = path.with_extension("json.tmp");
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let result = (|| -> io::Result<()> {
            use std::io::Write;
            let mut file = options.open(&temporary)?;
            file.write_all(&serde_json::to_vec(self)?)?;
            file.sync_all()?;
            fs::rename(&temporary, path)
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result
    }
    pub fn load_checkpoint(path: &Path) -> io::Result<Option<Self>> {
        match fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_require_owner_and_allow_offline() {
        let mut state = OobeState::default();
        state.advance_step(); state.advance_step(); state.advance_step();
        assert_eq!(state.current_step, OobeStep::OwnerSecurity);
        state.advance_step();
        assert_eq!(state.current_step, OobeStep::OwnerSecurity);
        assert!(!state.configure_owner("123"));
        assert!(state.configure_owner("1234"));
        state.advance_step();
        assert!(state.pairing_qr_ready);
        state.advance_step();
        assert!(state.completed);
        state.go_back();
        assert!(!state.completed);
    }
    #[test]
    fn checkpoint_round_trip() {
        let path = std::env::temp_dir().join(format!("hh-oobe-{}.json", std::process::id()));
        let mut state = OobeState::default();
        assert!(state.select_language("hi"));
        state.save_checkpoint(&path).unwrap();
        assert_eq!(OobeState::load_checkpoint(&path).unwrap(), Some(state));
        fs::remove_file(path).unwrap();
    }
}
