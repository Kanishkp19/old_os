//! Capture grant coordinator. Portal/PipeWire creation remains unavailable
//! until a validated backend can provide and revoke actual stream handles.

use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq)]
pub enum CaptureError {
    Unavailable,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "labwc portal/PipeWire capture backend is unavailable")
    }
}

impl std::error::Error for CaptureError {}

#[derive(Debug)]
pub struct CaptureGrant {
    pub app_id: String,
}

#[derive(Debug, Default)]
pub struct CaptureManager {
    active_captures: HashMap<String, CaptureGrant>,
}

impl CaptureManager {
    pub fn new() -> Self { Self::default() }

    /// No grant is issued without a real portal session and PipeWire node.
    pub fn grant_capture(&mut self, _app_id: &str) -> Result<String, CaptureError> {
        Err(CaptureError::Unavailable)
    }

    pub fn revoke_capture(&mut self, grant_id: &str) -> bool {
        self.active_captures.remove(grant_id).is_some()
    }

    pub fn teardown_all(&mut self) -> usize {
        let count = self.active_captures.len();
        self.active_captures.clear();
        count
    }

    pub fn on_session_lock(&mut self) {
        let revoked = self.teardown_all();
        tracing::info!(revoked, "capture grants torn down for session lock");
    }

    pub fn on_secure_prompt(&mut self) { self.on_session_lock(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_clears_all_grants() {
        let mut manager = CaptureManager::new();
        manager.active_captures.insert("one".into(), CaptureGrant { app_id: "app".into() });
        manager.active_captures.insert("two".into(), CaptureGrant { app_id: "app".into() });
        manager.on_session_lock();
        assert!(manager.active_captures.is_empty());
        assert!(!manager.revoke_capture("one"));
    }

    #[test]
    fn unsupported_backend_never_grants_capture() {
        let mut manager = CaptureManager::new();
        assert_eq!(manager.grant_capture("app"), Err(CaptureError::Unavailable));
    }
}
