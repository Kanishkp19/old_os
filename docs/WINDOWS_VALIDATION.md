# Windows validation ledger

Status: implementation and local verification in progress. These checks are not comprehensive product acceptance.

No Windows hardware, Apple signing, release signatures, soak, crash matrix or household pilot results are claimed.

| ID | Scenario | Result | Evidence / issue | Fix commit | Retest |
|---|---|---|---|---|---|
| ENV-001 | Windows physical validation | Pending | Current implementation host is macOS; user Windows laptop required | — | — |
| ENV-002 | Household pilot | Pending | Four-week, 20-household pilot follows technical validation | — | — |
| BUILD-001 | Rust workspace on macOS | Passed locally | `cargo test --workspace --locked --offline`: all unit and doc tests passed; `cargo check --workspace --locked --offline` passed | — | — |
| BUILD-002 | Tauri native crate on macOS | Passed locally | `cargo check --manifest-path desktop/src-tauri/Cargo.toml --locked --offline` passed after icon and API fixes | — | — |
| BUILD-003 | Desktop frontend | Passed with warning | `npm run check` found 0 errors, 1 missing-caption warning for user videos; calculator tests 2/2 and production build passed | — | — |
| BUILD-004 | Android debug unit tests | Passed locally | `./gradlew :app:testDebugUnitTest` passed; physical device workflows untested | — | — |
| ENV-003 | Apple native builds | Pending | Full Xcode is absent on this macOS host; `xcodebuild -version` selects Command Line Tools only | — | — |
| ENV-004 | Rust lint and formatting | Pending | Installed Rust toolchain lacks `cargo-clippy` and `cargo-fmt`; availability must be resolved before release | — | — |
| ENV-005 | Windows package and runtime | Pending | Mac host cannot execute Inno, PowerShell runtime, WebView2, Media Foundation or Windows service/login tests | — | — |
| ENV-006 | GitHub Windows build dispatch | Pending | Source branch is pushed, but `gh auth status` reports no GitHub CLI login on this host; manual workflow not dispatched | — | — |
| ENV-007 | Apple shared Swift typecheck | Blocked by local toolchain | Command Line Tools `swiftc` reports duplicate `SwiftBridging` module maps before source typechecking; full Xcode still required | — | — |
| SEC-001 | Transfer path ownership on the companion API | Passed locally | `cargo test -p hh-net --locked --offline transfer_authorization_tests` passes for owner, another device and unknown transfer on status, chunk and completion paths; real mTLS request test pending | — | Passed locally |
| BUILD-005 | Apple project-file syntax | Passed locally | `plutil -lint` passes both Xcode project files and three app/extension Info.plists; `python3 -m py_compile apple-shared/generate-projects.py` passes | — | — |
| BUILD-006 | Android debug APK packaging | Passed locally | `./gradlew :app:assembleDebug --offline` completed with 41 actionable tasks; generated APK remains a local unsigned/debug artifact, not a hardware result | — | — |
| STORE-001 | Corrupt second-copy repair and coverage | Passed locally | `cargo test -p hh-storage --locked --offline` passes 3 unit tests, including verified replacement, rejection of changed source, and per-file coverage freshness. Named-backup `ReplaceFileW` failure recovery and external-drive end-to-end recovery require Windows fault injection | — | Passed locally |
| APP-001 | Clear private browser history | Passed locally | `cargo test --manifest-path desktop/src-tauri/Cargo.toml --locked --offline clearing_browser_history_removes_active_and_trashed_rows_only` passes. Active and trashed browser rows are removed while Notes, bookmarks and download records remain. Closed-tab WebView2 profile removal still requires Windows validation | — | Passed locally |
| APP-002 | Browser tab session restoration | Passed locally; Windows pending | Private SQLite session roundtrip and unsafe-URL rejection pass in the native desktop tests. `npm run check` has 0 errors and the existing video-caption warning; `npm test` passes 2 calculator tests. Actual WebView2 restart and partial-failure recovery require Windows validation | — | Passed locally |
| APP-003 | Browser address/search behavior | Passed locally; Windows pending | Focused native test passes for phrase and `site:` searches, normal addresses, and rejection of explicit insecure/native/local addresses. `npm run check` has 0 errors and the existing caption warning; `npm test` passes 2/2; `npm run build` passes. Real website and default-browser behavior require Windows validation | — | Passed locally |
