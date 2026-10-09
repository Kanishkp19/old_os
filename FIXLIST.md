# Deferred defects found during BUILD

- `android/app/src/test/java/com/homehub/net/QrPayloadTest.kt:11`: Existing test expects acceptance of a legacy 16-hex QR fingerprint; Android now requires the full fingerprint for token submission. Update this test in the TEST pass. Severity: low.
