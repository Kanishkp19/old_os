# Deferred defects found during BUILD

- `android/app/src/test/java/com/homehub/net/QrPayloadTest.kt:11`: Existing test expects acceptance of a legacy 16-hex QR fingerprint; Android now requires the full fingerprint for token submission. Update this test in the TEST pass. Severity: low.
- `hub/crates/hh-net/src/routes.rs:418`: Transfer status, chunk upload and abort routes check the transfer scope but not transfer ownership; another paired device with a known transfer ID may access them. Fix in Phase 2 security pass. Severity: high.
- `desktop/src/App.svelte:248`: Existing video viewer has no caption track; `svelte-check` reports an accessibility warning. Add captions support or an explicit transcript affordance in the TEST pass. Severity: low.
