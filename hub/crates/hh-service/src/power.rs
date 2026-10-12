//! State and consent policy for service power transitions.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PowerState {
    #[default]
    Active,
    PreparingSleep,
    Suspended,
}

#[derive(Debug, PartialEq, Eq)]
pub enum PowerError {
    InvalidTransition { from: PowerState, to: PowerState },
    Inhibition(String),
}

impl std::fmt::Display for PowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid power state transition from {from:?} to {to:?}")
            }
            Self::Inhibition(message) => write!(f, "sleep inhibition failed: {message}"),
        }
    }
}

impl std::error::Error for PowerError {}

#[derive(Debug, Default)]
pub struct PowerCoordinator {
    state: PowerState,
    sleep_inhibit_active: bool,
    remote_input_revoked: bool,
    capture_revoked: bool,
}

impl PowerCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> PowerState {
        self.state
    }

    pub fn is_sleep_inhibit_active(&self) -> bool {
        self.sleep_inhibit_active
    }

    pub fn is_remote_input_revoked(&self) -> bool {
        self.remote_input_revoked
    }

    pub fn is_capture_revoked(&self) -> bool {
        self.capture_revoked
    }

    pub fn prepare_sleep(&mut self) -> Result<(), PowerError> {
        self.transition(PowerState::Active, PowerState::PreparingSleep)?;
        self.sleep_inhibit_active = true;
        self.remote_input_revoked = true;
        self.capture_revoked = true;
        Ok(())
    }

    pub fn on_suspend(&mut self) -> Result<(), PowerError> {
        self.transition(PowerState::PreparingSleep, PowerState::Suspended)
    }

    pub fn on_wake(&mut self) -> Result<(), PowerError> {
        if !matches!(
            self.state,
            PowerState::Suspended | PowerState::PreparingSleep
        ) {
            return Err(PowerError::InvalidTransition {
                from: self.state,
                to: PowerState::Active,
            });
        }
        self.state = PowerState::Active;
        self.sleep_inhibit_active = false;
        // Consent is never restored by a power transition.
        Ok(())
    }

    pub fn reauthorize_remote_input(&mut self) {
        self.remote_input_revoked = false;
    }

    pub fn reauthorize_capture(&mut self) {
        self.capture_revoked = false;
    }

    fn transition(&mut self, from: PowerState, to: PowerState) -> Result<(), PowerError> {
        if self.state != from {
            return Err(PowerError::InvalidTransition {
                from: self.state,
                to,
            });
        }
        self.state = to;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_sleep_arms_inhibition_and_revokes_grants() {
        let mut power = PowerCoordinator::new();
        power.prepare_sleep().unwrap();
        assert_eq!(power.state(), PowerState::PreparingSleep);
        assert!(power.is_sleep_inhibit_active());
        assert!(power.is_remote_input_revoked());
        assert!(power.is_capture_revoked());
    }

    #[test]
    fn wake_requires_fresh_consent() {
        let mut power = PowerCoordinator::new();
        power.prepare_sleep().unwrap();
        power.on_suspend().unwrap();
        power.on_wake().unwrap();
        assert_eq!(power.state(), PowerState::Active);
        assert!(!power.is_sleep_inhibit_active());
        assert!(power.is_remote_input_revoked());
        assert!(power.is_capture_revoked());
        power.reauthorize_remote_input();
        power.reauthorize_capture();
        assert!(!power.is_remote_input_revoked());
        assert!(!power.is_capture_revoked());
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        let mut power = PowerCoordinator::new();
        assert!(matches!(
            power.on_suspend(),
            Err(PowerError::InvalidTransition { .. })
        ));
        assert!(matches!(
            power.on_wake(),
            Err(PowerError::InvalidTransition { .. })
        ));
        power.prepare_sleep().unwrap();
        assert!(matches!(
            power.prepare_sleep(),
            Err(PowerError::InvalidTransition { .. })
        ));
    }
}
