//! Owner secret verification and local attempt throttling.

use rand::{rngs::OsRng, RngCore};
use std::time::{Duration, Instant};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("authentication rate limited for {seconds_remaining} seconds")]
    RateLimited { seconds_remaining: u64 },
}

pub struct LockAuth {
    owner_hash: String,
    salt: String,
    failed_attempts: u32,
    locked_until: Option<Instant>,
}

impl std::fmt::Debug for LockAuth {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LockAuth")
            .field("failed_attempts", &self.failed_attempts)
            .field("locked_until", &self.locked_until)
            .finish_non_exhaustive()
    }
}

impl LockAuth {
    pub fn new(pin_or_pass: &str) -> Self {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        let salt = hex(&salt);
        Self {
            owner_hash: hash(&salt, pin_or_pass),
            salt,
            failed_attempts: 0,
            locked_until: None,
        }
    }

    pub fn verify(&mut self, candidate: &str) -> Result<bool, AuthError> {
        if let Some(until) = self.locked_until {
            if let Some(remaining) = until.checked_duration_since(Instant::now()) {
                return Err(AuthError::RateLimited {
                    seconds_remaining: remaining.as_secs().saturating_add(1),
                });
            }
        }
        let candidate_hash = hash(&self.salt, candidate);
        let difference = candidate_hash
            .bytes()
            .zip(self.owner_hash.bytes())
            .fold(0u8, |difference, (a, b)| difference | (a ^ b));
        if difference == 0 {
            self.failed_attempts = 0;
            self.locked_until = None;
            return Ok(true);
        }
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.failed_attempts >= 5 {
            let shift = (self.failed_attempts - 5).min(4);
            let seconds = (5u64 << shift).min(60);
            self.locked_until = Some(Instant::now() + Duration::from_secs(seconds));
        }
        Ok(false)
    }
}

fn hash(salt: &str, secret: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(salt.as_bytes());
    hasher.update(secret.as_bytes());
    hasher.finalize().to_hex().to_string()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 15) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_candidate_unlocks() {
        let mut auth = LockAuth::new("correct");
        assert_eq!(auth.verify("correct"), Ok(true));
        assert_eq!(auth.failed_attempts, 0);
    }

    #[test]
    fn incorrect_candidate_fails() {
        assert_eq!(LockAuth::new("correct").verify("wrong"), Ok(false));
    }

    #[test]
    fn fifth_failure_rate_limits_following_attempt() {
        let mut auth = LockAuth::new("correct");
        for _ in 0..5 {
            assert_eq!(auth.verify("wrong"), Ok(false));
        }
        assert!(matches!(
            auth.verify("correct"),
            Err(AuthError::RateLimited { .. })
        ));
    }
}
