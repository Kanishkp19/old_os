//! Typed errors. Mapped to API error codes in API_SPEC §1 by hh-net.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("missing scope: {0}")]
    ForbiddenScope(String),
    #[error("device revoked")]
    DeviceRevoked,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("chunk hash mismatch at chunk {chunk}")]
    HashMismatch { chunk: u64 },
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("transfer gone or expired")]
    TransferGone,
    #[error("too large: {0}")]
    TooLarge(String),
    #[error("whole-file verification failed (root hash mismatch)")]
    RootHashMismatch,
    #[error("pairing locked after too many attempts")]
    PairingLocked,
    #[error("rate limited")]
    RateLimited,
    #[error("storage full")]
    StorageFull,
    #[error("storage unavailable: {0}")]
    StorageUnavailable(String),
    #[error("invalid pairing token")]
    InvalidToken,
    #[error("pairing token expired")]
    TokenExpired,
    #[error("pairing window is not open")]
    PairingClosed,
    #[error("database error: {0}")]
    Db(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// API error code string per API_SPEC §1.
impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::BadRequest(_) => "BAD_REQUEST",
            Error::Unauthenticated => "UNAUTHENTICATED",
            Error::ForbiddenScope(_) => "FORBIDDEN_SCOPE",
            Error::DeviceRevoked => "DEVICE_REVOKED",
            Error::NotFound(_) => "NOT_FOUND",
            Error::HashMismatch { .. } => "HASH_MISMATCH",
            Error::Conflict(_) => "CONFLICT",
            Error::TransferGone => "TRANSFER_GONE",
            Error::TooLarge(_) => "TOO_LARGE",
            Error::RootHashMismatch => "ROOT_HASH_MISMATCH",
            Error::PairingLocked => "PAIRING_LOCKED",
            Error::RateLimited => "RATE_LIMITED",
            Error::StorageFull => "STORAGE_FULL",
            Error::StorageUnavailable(_) => "STORAGE_UNAVAILABLE",
            Error::InvalidToken => "INVALID_TOKEN",
            Error::TokenExpired => "TOKEN_EXPIRED",
            Error::PairingClosed => "PAIRING_CLOSED",
            Error::Db(_) | Error::Io(_) | Error::Crypto(_) | Error::Internal(_) => "INTERNAL",
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Error::BadRequest(_) => 400,
            Error::Unauthenticated => 401,
            Error::ForbiddenScope(_) | Error::DeviceRevoked => 403,
            Error::NotFound(_) => 404,
            Error::HashMismatch { .. } | Error::Conflict(_) => 409,
            Error::TransferGone | Error::TokenExpired => 410,
            Error::TooLarge(_) => 413,
            Error::RootHashMismatch => 422,
            Error::PairingLocked => 423,
            Error::RateLimited => 429,
            Error::StorageFull => 507,
            Error::StorageUnavailable(_) => 503,
            Error::InvalidToken | Error::PairingClosed => 401,
            Error::Db(_) | Error::Io(_) | Error::Crypto(_) | Error::Internal(_) => 500,
        }
    }

    /// Clients may safely retry retryable errors (API_SPEC §1 `retryable`).
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Error::HashMismatch { .. }
                | Error::RateLimited
                | Error::StorageUnavailable(_)
                | Error::Io(_)
        )
    }
}
