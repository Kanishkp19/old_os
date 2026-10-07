# Home Hub iOS (M3 MVP skeleton)

Scope for Phase 2: QR pairing, share-extension queue, background photo backup.
Gallery and remote control are intentionally out of scope for the MVP.

## Building

Open this folder in Xcode 16+ and create two targets:

1. **App target** `HomeHub` — add all files in `HomeHub/`.
   Capabilities: App Groups (`group.com.homehub`), Background Modes
   (fetch, processing), Keychain Sharing not required.
2. **Share extension target** `HomeHubShare` — add `HomeHubShare/`,
   share the App Group with the app target.

The hub-side protocol is unchanged — iOS speaks the same API_SPEC v1 as
Android and macOS. Pairing on iOS mirrors `android/.../PairingClient.kt`:
generate a P-256 key in the Keychain, pin the CA fingerprint from the QR
*before* sending the one-time token, POST the CSR to `https://<hub>:47802/pair`.

## Status

This is a bring-up skeleton for the M3 pilot: `PairingClient.pair` throws
"not implemented" by design so the app cannot be mistaken for a shipping
client. The Android and macOS clients are the reference implementations.
