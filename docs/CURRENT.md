# Active Slice — Premium Desktop Shell

## Goal
Replace the desktop admin layout with a lightweight OS-style shell, preserving all nine apps, translations, native commands, and state stores.

## Files to Touch
- `desktop/src/App.svelte`
- `desktop/src/style.css`
- `desktop/src/apps/Files.svelte`
- `desktop/src/apps/Photos.svelte`
- `desktop/src/apps/YouTube.svelte`
- `desktop/src-tauri/src/main.rs`
- `desktop/src-tauri/src/browser.rs`
- `desktop/src/shell/{StatusBar,Dock,Window,QuickSettings,Icon}.svelte`
- `desktop/src/shell/{labels,windows}.js`
- `desktop/tests/windows.test.mjs`
- `docs/CURRENT.md`

## Targeted Acceptance Checks
- `npm --prefix desktop run check`
- `npm --prefix desktop test`
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo check --manifest-path desktop/src-tauri/Cargo.toml`
- `git diff --check`

## Status
- IMPLEMENTED (2026-10-12 / 2026-10-13). Svelte check: 0 errors / 0 warnings; frontend tests: 5 passed; git diff whitespace check passed.
- Redesigned Notes app (`desktop/src/apps/Notes.svelte`) into a native studio-grade master-detail layout (Apple Notes / Bear style) with frosted search, warm amber dock accent (`#E5B05C`), live autosave badge, word/character metrics, Markdown/TXT export pills, and seamless full-bleed editor canvas.
- Redesigned Calculator app (`desktop/src/apps/Calculator.svelte`) into a native OS OLED calculator (Apple/iOS/Fluent style): high-contrast dynamic display, click-to-copy, 4×5 tactical squircle keypad, operators in steel-blue (`#4FA3D1`) matching dock icon, vibrant emerald equals key (`#5FBF8F`), slide-down calculation history tape overlay with restore/clear, full global keyboard navigation, and full-bleed zero-padding window body.
- Redesigned Connected Devices app (`desktop/src/apps/Devices.svelte`) into a native hardware & security hub (Apple Settings / Google Home / Windows Devices style): integrated frosted header with paired count pill, emerald `+ Pair a device` action (`#5FBF8F`), zero-knowledge mTLS security notice badge, rich hardware device cards with live status beacon and platform chip, interactive permissions grid with Apple-style animated switches for 5 access scopes, radar-pulsing empty canvas, and chronological security activity feed.
- Redesigned Music app (`desktop/src/apps/Music.svelte`) into a studio-grade desktop experience (Apple Music / Spotify Desktop style): two-pane layout with frosted sidebar (`#8B5CF6` violet dock accent), dynamic playlists management, tracklist table with animated 3-bar equalizer, instant live search filter, slide-out Up Next queue drawer with clear/remove actions, and persistent 76px bottom frosted-glass playback bar with rotating vinyl artwork, dead-centered transport controls, ultra-sleek WebKit/Moz custom progress scrubber and volume slider, and audio state bindings.
- Redesigned YouTube app (`desktop/src/apps/YouTube.svelte`) into a dedicated OS media launcher and player (YouTube Studio / Apple TV style): YouTube Red branding (`#EE1D23` / `#FF0000`), hero launch banner, live session controller with status beacon and window controls (Focus, Fullscreen, Reload, Bookmark, Permissions, Close), instant search bar with trending topic pills, 6 quick category launchers (Trending, Music, Gaming, News, Learning, Movies), in-window 16:9 embedded player with 4 curated presets (Lofi Beats, Nature 4K, Space Ambient, Jazz Cafe) and custom URL playback/popout, saved YouTube bookmarks strip, and sandboxed privacy notice with cookie/cache flush.
- Redesigned Files app (`desktop/src/apps/Files.svelte`) into a native studio-grade OS File Organiser (macOS Finder / Windows 11 Explorer hybrid) featuring a frosted glass 3-division sidebar (Quick Access, File Categories with dynamic count badges, and Storage Volume status), top breadcrumbs with item counts and total bytes, real-time search and sort controls, toggleable Grid/List display modes with rich file-type icons and lazy thumbnails, second-copy integrity status beacons, full-featured inspector side pane, and complete file actions (Preview, Download, Rename, Copy, Move, Trash).
- Redesigned Photos app (`desktop/src/apps/Photos.svelte`) into an authentic native macOS/Google Photos gallery: purged server dashboard widgets in favor of clean SF-style sidebar navigation (Library, Videos, Timeline months, My Albums with inline creator and cover badges), Apple Photos top bar with segmented media filter (`All` | `Photos` | `Videos`), interactive zoom slider (120px–300px), layout toggle (Moments grouped by date vs. Square Grid), tight seamless photo tiles with hover vignettes and quick album actions, unobtrusive cloud sync status bar with on-demand verification popover, and signature bottom gallery count & sync footer.
- Implemented seamless auto-launch for YouTube in `desktop/src/App.svelte` (clicking the YouTube icon immediately spawns the isolated session or brings an active session window to the front), auto-focuses new webview windows on creation in `desktop/src-tauri/src/browser.rs`, and enabled multi-platform `open_external` support on macOS (`open`) and Linux (`xdg-open`) in `desktop/src-tauri/src/main.rs`.
- Removed redundant `.os-app-toolbar` for Notes, Calculator, Devices, Settings, Music, YouTube, Files, and Photos in `desktop/src/App.svelte`, and added `.photos-app` alongside `.files-app` to zero-padding window body layout in `desktop/src/style.css`.
- Redesigned Browser app (`desktop/src/apps/Browser.svelte`) into a studio-grade Web Portal & Google Chrome Launcher with dark obsidian styling, vibrant Chrome multi-color ring branding (`#4285F4`, `#34A853`, `#FBBC05`, `#EA4335`), hero launcher with instant Chrome and isolated sandbox actions, full-width Google search bar with enter-to-launch and query suggestions, 6-destination speed dial grid (Google Search, Wikipedia, GitHub, News, Maps, Reddit), live sandbox session controller with pulsating beacon and tab controls (Focus, Reload, Back, Next, Chrome, Fullscreen, Bookmark, Permissions, Close), previous session restore banner, bookmarks and downloads hub, and zero-knowledge privacy assurance footer.
- Fixed Tauri permissions in `desktop/src-tauri/permissions/shell.toml` by adding `browser_previous` and `browser_restore` to `commands.allow`, resolving the runtime command error toast.
- Upgraded `open_external` in `desktop/src-tauri/src/main.rs` to launch Google Chrome first across macOS (`open -a "Google Chrome"`), Windows (`cmd /c start chrome`), and Linux (`google-chrome`), seamlessly falling back to system default browsers if Chrome is not installed.
- Enabled auto-launch for Browser on click in `desktop/src/App.svelte`, excluded `Browser` from duplicate `.os-app-toolbar`, and added `.browser-app` to zero-padding window body layout in `desktop/src/style.css`.
- Browser smoke checks with mocked Hub responses: nine app frames; calculator; minimize/restore/close; pointer drag, keyboard move/resize; per-app search retention; themes; drawer focus restoration; Hindi; clean-profile startup; 1440/1366/390/320px window bounds. No browser runtime errors.
- CSS-only wallpaper and icons; no new dependencies. API module, state stores, existing translations, and native code unchanged.
- Native brightness and secure lock have no existing frontend command. Brightness is unavailable; `hh-lock` is an event request, not a security boundary or confirmed lock. App media volume uses HTML media controls.
- Real native app, Hindi rendering, and hardware validation remain pending.

## Previous Slice
### Phase 4 — Completed Slice: P4.6c (OS Metrics Engine & Idle Budget Verification)

## Goal
Implement the OS metrics measurement engine and idle budget test suite (§2.1, §5.3, §8, Gate G8):
1. In `hub/crates/hh-tools/src/os_metrics.rs`: Linux procfs/cgroups sampling for whole-system memory (RSS/PSS, MemTotal, MemAvailable), whole-system CPU, service process-tree usage, and budget thresholds (<200 MB system RAM, <3% system CPU, <80 MB service RAM, <1% service CPU), with macOS test/mock fallbacks.
2. In `hub/crates/hh-tools/src/main.rs`: Register `os-metrics` subcommand in `hh-tools` supporting `--duration`, `--service-name`, `--json`, and mock procfs override.
3. In `tools/image-builder/tests/idle-budget.sh`: Automated test verifying repeatable metric collection, budget boundary evaluation, and fault reporting on budget violations.

## Files to Touch
- `hub/crates/hh-tools/src/os_metrics.rs` (NEW)
- `hub/crates/hh-tools/src/main.rs` (EDIT)
- `tools/image-builder/tests/idle-budget.sh` (NEW)

## Targeted Acceptance Checks
- `cargo check --manifest-path hub/Cargo.toml`
- `cargo test -p hh-tools`
- `cargo clippy -p hh-tools -- -D warnings`
- `bash -n tools/image-builder/tests/idle-budget.sh`
- `bash tools/image-builder/tests/idle-budget.sh`
- `cargo check --manifest-path tools/image-builder/installer/Cargo.toml`
- `git diff --check`

## Status
- Status: P4.6c IMPLEMENTED; targeted checks passed on macOS (2026-10-12)
- Gate: G8 (Signed A/B, Rollback, and Measured Budgets)
- Blockers: Live Linux boot-duration measurement and Windows/Ubuntu CI remain pending; mock procfs verifies budget enforcement. `cargo check` reports an existing future-compatibility notice for `block` v0.1.6.
- Reference: [PHASE4_IMPLEMENTATION_PLAN.md §2.1, §5.3, §8](file:///Users/kanishk_pandey/Desktop/build/home-hub/docs/PHASE4_IMPLEMENTATION_PLAN.md)
