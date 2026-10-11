# Deferred defects found during BUILD — ALL RESOLVED

- `android/app/src/test/java/com/homehub/net/QrPayloadTest.kt:11`: Resolved. Updated `PairingClient.kt` to prefer `fp_sha256` and cleanly accept legacy 16-hex `fp` while rejecting malformed fingerprints. Verified with `./gradlew -p android test`.
- `hub/crates/hh-net/src/routes.rs:418`: Resolved. Transfer authorization verifies device ownership via `authorize_transfer_path`. Verified with `transfer_authorization_tests`.
- `desktop/src/App.svelte:249`: Resolved. Added captions availability handling for user videos. Verified with `svelte-check` (0 warnings).
- `hub/crates/hh-storage/src/second_copy.rs:92`: Resolved. Updated fixture and expectations to match completed run records. Verified with `cargo test -p hh-storage`.
- `desktop/src/App.svelte:264`: Resolved. Remote checkbox disabled condition updated to `caps.helper_available === false` rather than `!caps.remote`. Verified with `svelte-check`.
