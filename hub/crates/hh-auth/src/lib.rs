//! hh-auth: Hub CA, pairing, device certificates, revocation (TRD §5, SECURITY §4–6).

pub mod ca;
pub mod pairing;
pub mod revoke;

pub use ca::{hub_fingerprint, HubIdentity};
pub use pairing::{PairingManager, PairingWindow};
pub use revoke::RevocationList;
