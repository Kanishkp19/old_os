# Android companion implementation checkpoint

Source implementation only. No Gradle tasks, test execution, build, formatting or device verification has run during the deferred implementation phase. The old demonstrated APK remains separate.

The companion now includes pinned bootstrap pairing (prefers full SHA-256, preserves the legacy prefix), durable source-safe upload queues and Home-scoped recovery, MediaStore backup with Android 14 selected-media access, schedules and constraints, and system-confirmed cleanup. Cleanup persists a review nonce and source snapshot before requesting a server lease, retries the same review after a lost response, rehashes after the in-app review, retains pins through cancellation/process death, and exposes explicit orphan-review recovery. It never directly deletes phone media.

Certificate renewal persists a new Keystore key and exact CSR before request, validates and persists the returned certificate before its activation request, and retains the old identity until the Hub authenticates the new identity. The backend must replay the same CSR/certificate while staged, and authorize that staged identity at its first authenticated request. Expired/revoked identities require scanning a new pairing code.

Files use server cursor sorting/search/category filtering. Gallery includes year/month navigation, a zoomable large preview, metadata, verified original export/open/share and unavailable-format fallback. Streaming downloads preserve private partials across failure/restart, validate Range continuation and object identity, and hash the complete local copy before any document/provider export. Sharing grants read access only to the verified app-private copy. Relay selectors use `/v1/relay/devices`; sending an existing file uses `/v1/files/{id}/relay` and is ownership checked by Home.

Native WebRTC has explicit receive-only viewing and send-only casting, no ICE servers or microphone, numeric private/link-local host candidate filtering, bounded offer/gather waits, periodic authenticated heartbeat, and deterministic stop/release. Casting requires a fresh Android MediaProjection consent and its foreground service. Remote input/power is separately permission scoped and destructive power actions have a confirmation.

English/Hindi copy, language selection, transfer history/cancellation/progress, completion notifications, Android permission settings/reselection and authenticated connectivity/discovery recovery are wired into the Compose navigation.

Authored, unrun tests cover QR full/legacy pins and malformed/non-local inputs, exact Range headers, private ICE filtering, BLAKE3 vectors and tree-buffer boundaries, and a Room v1→v2 upgrade retaining sources and cancellation after worker recovery. Unit tasks and connected instrumentation must be run only after all project implementation phases finish.

## Required validation and known limits

- Run Kotlin/Compose/Room/Hilt compile and authored tests; no build success is claimed.
- Physically validate Android 9, 13, 14 and 15, selected media/permission revocation, OEM background restrictions, 20 GB transfers, lost responses, restart and disk-full recovery.
- Test certificate activation response loss, old-key rejection, pending renewal beyond the Hub seven-day stage lifetime, and pairing interrupted between credential/trust persistence. No live certificate fault-injection result exists yet.
- Test cleanup while the Android confirmation is open, both cancellation paths, process death and source mutation. Android URI-based confirmation does not offer a conditional hash-based deletion API: local content is freshly checked before launch, while the Hub copy stays pinned until explicit completion/recovery.
- When selected-media visibility cannot prove an item is absent after permission changes, cleanup does not claim local-freed status. Explicit recovery can release review pins without making that claim.
- Gallery zoom uses the 1024px preview. Verified original viewing/video playback is delegated to an installed compatible Android app; unsupported originals remain downloadable.
- Downloads retained in app-private cache can be evicted by Android, in which case they restart safely. Large verified sharing copies occupy local space and require storage/OEM validation. The pure Kotlin BLAKE3 implementation has not been performance benchmarked on reference phones.
- Physical Windows/Android casting, native codec behavior and revocation/disconnect timing remain pending; no synthetic screen fallback is used.

Android API references used during implementation: [selected photo access](https://developer.android.com/about/versions/14/changes/partial-photo-video-access), [MediaProjection consent and foreground-service lifecycle](https://developer.android.com/media/grow/media-projection).
