//! Session lock state and redacted public status.
//!
//! A compositor-backed session-lock protocol must own the actual lock surfaces.
//! This state model never treats a process restart or output loss as an unlock.

pub mod auth;

use auth::{AuthError, LockAuth};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockStatus {
    pub clock_text: String,
    pub redacted_notice: String,
    pub battery_text: Option<String>,
}

impl Default for LockStatus {
    fn default() -> Self {
        Self {
            clock_text: String::new(),
            redacted_notice: "Files protected".into(),
            battery_text: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    Locked { outputs: u32 },
    Authenticating,
    Unlocked,
}

#[derive(Debug)]
pub struct SessionLocker {
    auth: LockAuth,
    state: LockState,
    status: LockStatus,
    outputs: u32,
}

impl SessionLocker {
    pub fn new(owner_secret: &str) -> Self {
        Self {
            auth: LockAuth::new(owner_secret),
            state: LockState::Locked { outputs: 0 },
            status: LockStatus::default(),
            outputs: 0,
        }
    }

    pub fn state(&self) -> LockState {
        self.state
    }

    pub fn status(&self) -> &LockStatus {
        &self.status
    }

    pub fn set_clock(&mut self, clock_text: String) {
        self.status.clock_text = clock_text;
    }

    pub fn set_battery(&mut self, battery_text: Option<String>) {
        self.status.battery_text = battery_text;
    }

    /// Called only after a compositor lock surface is configured for every output.
    pub fn set_locked_outputs(&mut self, outputs: u32) {
        self.outputs = outputs;
        self.state = LockState::Locked { outputs };
    }

    pub fn begin_authentication(&mut self) -> bool {
        if self.outputs == 0 || !matches!(self.state, LockState::Locked { .. }) {
            return false;
        }
        self.state = LockState::Authenticating;
        true
    }

    pub fn verify(&mut self, candidate: &str) -> Result<bool, AuthError> {
        if self.state != LockState::Authenticating || self.outputs == 0 {
            return Ok(false);
        }
        match self.auth.verify(candidate) {
            Ok(true) => {
                self.state = LockState::Unlocked;
                Ok(true)
            }
            result => {
                self.state = LockState::Locked {
                    outputs: self.outputs,
                };
                result
            }
        }
    }

    pub fn compositor_failed(&mut self) {
        self.outputs = 0;
        self.state = LockState::Locked { outputs: 0 };
    }

    pub fn process_restarted(&mut self) {
        self.compositor_failed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_redacted() {
        let mut locker = SessionLocker::new("secret");
        locker.set_clock("12:34".into());
        let shown = format!("{:?}", locker.status());
        assert!(shown.contains("Files protected"));
        assert!(!shown.contains("secret"));
        assert!(!shown.contains("filename"));
    }

    #[test]
    fn unlock_requires_surfaces_and_owner_secret() {
        let mut locker = SessionLocker::new("secret");
        assert!(!locker.begin_authentication());
        locker.set_locked_outputs(2);
        assert!(locker.begin_authentication());
        assert_eq!(locker.verify("wrong"), Ok(false));
        assert_eq!(locker.state(), LockState::Locked { outputs: 2 });
        assert!(locker.begin_authentication());
        assert_eq!(locker.verify("secret"), Ok(true));
        assert_eq!(locker.state(), LockState::Unlocked);
        locker.compositor_failed();
        assert_eq!(locker.state(), LockState::Locked { outputs: 0 });
    }
}
