# Phase 4 — Home Hub OS UI/UX Design

Status: proposed implementation specification, 11 October 2026. This document designs a bootable Home Hub OS; it does not claim the OS exists or that hardware acceptance has passed. The requested planning work is separate from the completed E2E slice in `docs/CURRENT.md`.

## 1. Product direction and source of truth

The experience is **“my Home Computer”**: open the lid, unlock, find photos, play music, write a note, or connect a phone. User-facing language uses “Home Computer,” “Connected devices,” “Your files,” and “Second copy.” Technical terms in this document belong in engineering material, not ordinary product screens.

Preserve the existing green identity, English/Hindi strings, transfer safety, private app storage, and Rust services. Keep all existing network contracts and database migrations. The repository currently contains 14 backend crates and **13 SQL migration files**, not the 11 stated in the supplied prompt. Notes, playlists, bookmarks, and browser sessions already use a separate private `desktop.db`; they are not hub API features. Existing Windows validation is evidence about that product, not evidence of Linux readiness.

### 1.1 Inspiration translated into one coherent product

| Source | Adopt | Home Hub rule |
|---|---|---|
| Linux / GNOME / KDE | Discoverable overview, workspace switching, clear system ownership | Home overview plus two optional workspaces; advanced controls live in Settings |
| Windows | Familiar bottom taskbar, labelled window controls, snapping | Persistent app shelf; minimize/maximize/close on the right; explicit split-view action |
| macOS | Calm desktop, consistent materials, restrained motion, clear app focus | Quiet wallpaper, translucent chrome, one active-window treatment; no duplicate global menu bar |
| Android | Quick settings, grouped notifications, touch sizing, Back/Home/Recents | Large controls and stable navigation; touch mode exposes Back, Home, and Recents |

These are design influences, not pixel copies. Do not combine four competing navigation systems or reuse branded wallpapers, logos, or proprietary fonts. Default to a calm forest-at-dusk wallpaper with a low-detail center and subtle green horizon, bundled locally.

## 2. Shell architecture

Choose the hybrid architecture: labwc owns Wayland composition/window management; a native Rust shell supplies product chrome; reusable Svelte/Tauri content windows load on demand. A dedicated session-lock client supplies secure lock surfaces. The launcher, lock screen, status bar, notifications, quick settings, OOBE, and window controls remain usable without a WebView. Smithay is an optional separately estimated custom-compositor track, not a prerequisite for this UI. A hard Week 1 VM spike must prove the selected base image, native shell dependencies and real WebKitGTK app work together before commitment.

### 2.1 Desktop geometry and window policy

- At 1366×768 logical pixels: top status bar 40 px; bottom centered shelf 64 px high, 12 px from the bottom; 16 px safe edges. Work area excludes both. At 1920×1080, preserve sizes and increase whitespace.
- Left of status bar: Home glyph and “Home Computer”; middle: date and clock; right: device count, storage health, network, audio, battery when present, notification bell. Every glyph has a text tooltip and accessible name. Never show a phantom battery on a desktop.
- Shelf: Home, Photos, Files, Browser, Music, then running apps and a Recents button. Pins can be reordered with drag or a keyboard menu. Nine apps are always available in Home: Photos, Files, Browser, YouTube, Music, Notes, Calculator, Devices, Settings.
- Mouse default: one maximized window, with restore to floating available. Minimum floating window 480×360; default 960×640 constrained to the work area. Calculator opens 360×480 as the sole exception. Use right-hand controls in the order minimize, maximize/restore, close, with 40×40 targets.
- Split view is 50:50 or ⅓:⅔ with an 8 px divider and 24 px draggable hit area. Require at least 480 px per app; below 1024 px usable width, offer full-screen switching instead. Snap preview appears after 250 ms at an edge; keyboard and menu equivalents exist.
- Touch default: full-work-area apps, bottom navigation, no tiny resize handles. A keyboard/trackpad connection does not silently change the layout; suggest the other mode once.
- Two optional workspaces named “Home” and “Focus”; hide the workspace switcher until enabled. Windows are owned by a workspace, notifications by the owner profile. Settings persist per owner ID, never a hardcoded `user1`.
- Close saves state then destroys the app surface. Minimize preserves it. Suspend only idle, saved apps under memory pressure; never suspend active transfers, unsaved notes, audio, or screen sharing. Resume from a saved app state, with a visible loading state.
- Single-display v1: mirror to a supported external display, fitting the shared work area; extended desktops are deferred. Disconnect restores focus to the laptop. Lid-close sleep policy respects active transfers and explicit user preference.

**Boot input-mode decision:** enumerate physical input capabilities before the first interactive screen, waiting at most two seconds for initial discovery. Restore the owner's explicit `mouse` or `touch` preference first; otherwise `auto` selects touch when a touchscreen exists and tablet posture is active or no physical pointer exists. Select mouse when a mouse/trackpad is present in laptop posture, including a normal touchscreen laptop. A keyboard-only or unknown configuration defaults to mouse layout with complete keyboard navigation. Never infer touch from viewport width, user-agent or injected phone/uinput events.

OOBE offers a visible “Touch controls / Mouse controls” switch before any fine-target task. Store preference as `input.mode=auto|mouse|touch` in owner shell JSON. In auto mode, posture/hotplug changes offer a one-time non-modal switch prompt; they do not rearrange a form during editing. Manual selection persists across boots. If the only pointer disappears, keep keyboard navigation and offer touch layout immediately when a touchscreen is available; never strand the user behind a mouse-only dialog. The on-screen keyboard follows editable focus and physical-keyboard availability, independently of target-size mode. Test tablet-only, ordinary laptop, convertible, keyboard-only, delayed USB discovery and remote-only input. Baseline implementation belongs to `hh-shell/src/input_mode.rs`; optional Smithay branch uses `input.rs`.

### 2.2 Lock, boot, and recovery

Boot → data unlock if enabled → start service → first-run setup or owner lock → desktop. A native lock owns input and obscures all app surfaces before suspend, display reconnect, and capture restart. It is not a translucent overlay over readable content. Screen sharing and phone input stop at lock; backup may continue after data is mounted. Compositor failure returns to a locked session.

After 5 minutes without local input, dim; after 10 minutes, lock; after 20 minutes, blank the display. All are configurable. Playback prevents blanking when appropriate; it does not remove the explicit Lock action. An optional screensaver displays only clock and generic health. The lock shows “Files protected,” “2 devices connected,” and “Last backup: today, 18:42” only when owner-enabled; default hides names, thumbnails, note text, and device names.

## 3. Navigation and interaction model

| Action | Keyboard / pointer | Touch / phone equivalent |
|---|---|---|
| Home | Super, or Home glyph | Home button; swipe up from bottom edge |
| Find an app | Type while Home is open; Ctrl+Space anywhere | Tap Home search; on-screen keyboard |
| Switch app | Alt+Tab / Shift+Alt+Tab; click shelf | Recents; swipe up and hold |
| Close | Alt+F4; labelled close control | Close from app menu or Recents |
| Back within app | Alt+Left; app Back control | Back button or edge gesture where it does not conflict |
| Notifications / quick settings | Super+N / Super+A | Tap bell / status cluster; top-edge swipe opens notifications |
| Lock | Super+L | Quick settings → Lock; local keyboard needed to unlock |
| Snap / workspace | Super+Left/Right; Super+Ctrl+Left/Right | Window menu → Split; Recents workspace selector |
| Cancel | Escape | Back or explicit Cancel |
| App search / browser address | Ctrl+F / Ctrl+L in Browser | Labelled search/address field |

Super means Windows-logo key on PC keyboards or Command on Apple keyboards. App editing retains Ctrl shortcuts on the OS; Command aliases are configurable. Reserve shell combinations centrally; do not intercept keys while an input method is composing. The on-screen keyboard appears on editable focus in touch mode, includes Hindi input, and leaves the focused field visible. Every gesture has a visible alternative.

Notifications enter at upper right below the status bar, at most three visible, newest last, for 6 seconds; pause dismissal on hover/focus. Group repeated transfer completions by device/job. Ordinary banners do not steal focus. Critical storage failures remain in the center until acknowledged, but acknowledgement never marks the disk healthy. Do Not Disturb hides ordinary banners and sounds; critical storage warnings still appear without repeated sound. Clear all dismisses ordinary history, not unresolved safety conditions.

The device-count button opens a compact popover with connected and last-seen states, a Pair button, and “Open Devices.” Paired is not synonymous with currently connected. State older than 30 seconds is marked “Checking”; an API failure displays “Connection unavailable,” never zero devices or a healthy disk.

## 4. OS visual design system

### 4.1 Tokens

| Token | Dark default | Light option | Purpose |
|---|---|---|---|
| `os.wallpaper.base` | `#101A17` | `#E4ECE6` | Low-detail wallpaper base |
| `os.bg` / existing `--bg` | `#121413` | `#FAFAF7` | App canvas |
| `os.surface` / `--surface` | `#1B1E1C` | `#FFFFFF` | Menus, controls, app panes |
| `os.chrome` | `#1B1E1CF2` | `#FFFFFFF2` | Shelf and status bar, 95% opacity |
| `os.ink` / `--ink` | `#ECEFEA` | `#1A1D1B` | Primary text |
| `os.ink.secondary` | `#A3ABA5` | `#5B635E` | Supporting text |
| `os.accent` / `--accent` | `#5FBF8F` | `#2F6F4E` | Selected and primary actions |
| `os.accent.ink` | `#0B1A12` | `#FFFFFF` | Text on accent |
| `os.warning` | `#E5B05C` | `#875610` | Attention with icon and text |
| `os.danger` | `#F2877F` | `#B3261E` | Failure and destructive actions |
| `os.success` | `#6CC070` | `#2E7D32` | Verified success |
| `os.divider` | `#2A2E2B` | `#E3E6E1` | Decorative separators |
| `os.focus` | `#8CDEB3` | `#215E40` | 3 px focus ring, 2 px offset |

Controls requiring an identifiable boundary use a stronger outline than decorative dividers; verify 3:1 against adjacent surfaces. Text must meet 4.5:1, large text 3:1. Measure composited chrome against every bundled wallpaper. High contrast uses opaque black/white surfaces, explicit outlines, and no transparency. Blur is off by default for the resource target; optional 12 px blur is enabled only on validated hardware, cached when the scene is static.

- Bundle Roboto and Noto Sans Devanagari locally with licenses; no remote font fetches. Use 12/14/16/20/28/36 px scale, 16 px body, 1.4 line-height, 500/600 weight for labels/headings, tabular numerals for time and sizes. Hindi labels wrap rather than truncate core actions.
- Spacing: 4/8/12/16/24/32/48. Radii: 8 inputs, 12 panels, 16 app icons/shelf, 20 major overlays. Window chrome 12 radius; fullscreen/split interiors square at shared edges.
- Launcher icons: 64 px artwork in a 96×104 tile; touch tile 104×112. Use the existing Lucide/Material-style outline vocabulary for system icons at 20/24 px; app icons have distinct colored geometric backgrounds and readable labels. Never use emoji as UI assets.
- All touch targets ≥48×48; desktop controls ≥40×40, dense list row ≥40 high; touch row ≥56. Selection has fill plus mark; hover has subtle fill; pressed slightly darkens; disabled retains readable text plus an explanation on focus. Keyboard focus is independent of selection.
- Native and Svelte tokens are generated from the same proposed `desktop/src/theme/os-tokens.json`; do not hand-maintain two palettes. Local scale options 100/125/150/200%, text size 100/125/150/200%; layout reflows at 200%.

### 4.2 Motion and sound

| Event | Motion | Reduced-motion behavior |
|---|---|---|
| Boot | Static glyph; progress text changes without invented percentages | Identical |
| App launch / switch | 180 ms opacity and 8 px travel / 120 ms fade | Immediate focus, no travel |
| Home / Recents | 180 ms ease-out, restrained scale 0.98→1 | 80 ms fade |
| Notification / panel | 160 ms, 16 px edge translation | 80 ms fade |
| Lock / unlock | 200 ms crossfade after input/capture lock takes effect | Immediate transition |
| Window snap | 150 ms geometry transition | Immediate geometry |

No continuous wallpaper animation, breathing status indicators, or polling-driven redraws. Sounds default off except optional accessibility cues. Errors never depend on sound.

### 4.3 Asset production and packaging

Keep original SVG masters under `desktop/assets/os/source/`, including wallpaper, nine individual app icons and a symbolic system-icon sheet. `manifest.json` records asset IDs, authors/licenses, source hashes, variants and translation keys; `LICENSES.md` and locally bundled font licenses travel with the image. `desktop/scripts/build-os-assets.mjs` sanitizes SVG and deterministically produces light/dark/high-contrast variants, 32–256 px app icons, and 1280×800/1920×1080/3840×2160 wallpaper exports using pinned build tools. Use one palette and the same asset IDs in native and Svelte surfaces. Runtime never downloads artwork or fonts.

`check-os-assets.mjs` verifies dimensions, hashes, all nine apps, font/license coverage and absence of scripts/external SVG resources. Designer reviews a contact sheet, 16:9/16:10 center crops, safe areas, Hindi labels, 200% scaling and composed contrast. Package only reviewed exports, decode only the active wallpaper size, and include texture memory in measurements. A missing file displays a generic icon. App artwork is original; the YouTube launcher may use a generic video symbol rather than unapproved brand artwork.

## 5. Backend and local capability contract

All paths below are existing unless explicitly marked **NEW LOCAL**. `/api/*` is the authenticated loopback dashboard on 127.0.0.1:47801. Trusted apps use `desktop/src/api.js` → Tauri `hub_request` → the fixed-loopback proxy with the existing local token; the token never enters page JavaScript. Native shell reads through a new owner-authorized local broker. `/v1/*` stays mTLS on 47800, `/pair` stays time-boxed on 47802. Native app commands refer to `desktop/src-tauri/src/main.rs`; OS operations are not invented REST routes.

**NEW LOCAL** means a typed, Unix-socket request to `hh-session` or the proposed shell broker, protected by peer credentials, session ownership, operation allowlists, and request limits. Wi-Fi/logind/audio use narrowly allowed system D-Bus operations. No extra TCP port and no broad D-Bus access for webpages. New local methods: `shell.snapshot`, `os.network.list/connect/disconnect`, `os.audio.set`, `os.display.set`, `os.power.request`, `owner.unlock/lock`, `shell.preferences.get/set`, `shell.notifications.list/dismiss`, `os.update.status/install`, `migration.status/start`, `albums.list/create/update/delete`, `albums.items.list/add/remove`, and `backup.summary`. Album operations reuse existing hub tables through a service-owned broker; backup summary delegates to existing photo service logic. These local methods require implementation; they are not currently supported.

For each app: loading skeleton retains navigation; empty state offers one useful action; errors retain prior data labelled stale and an explicit Retry; permission denial explains how to enable access without retry loops. Data mutations use pending/succeeded/failed states and cannot show success before acknowledgement.

## 6. Screen-by-screen specification

### S01. Boot splash → OOBE → lock

```text
BOOT                         OOBE                           LOCK
       [house glyph]          Set up your Home Computer      18:42
       Home Computer         Language > Network > Restore   Sunday, 11 October
       Starting your files…  > Owner > Devices > Ready      [Owner] [Unlock]
                             [Back]             [Continue]  Files protected
       [Recovery options]                                   [Accessibility] [Power]
```

Hierarchy: brand and truthful startup phase; then one setup decision per screen; then clock and owner unlock. Boot has no fake progress; after 15 seconds show the actual waiting phase; after 60 seconds offer Retry and Recovery while retaining diagnostics. OOBE steps: language/accessibility, hardware compatibility, network (skip allowed), new setup or verified migration, owner passphrase/recovery key, privacy/power/update preferences, optional phone pairing, Ready. Installation disk choices occur in the installer before this flow. Persist each completed step atomically; never create a fresh identity over a pending migration. “No internet” permits local setup and files.

States: starting, storage locked, setup pending, migration pending, ready, wrong passphrase with increasing delay, recovery needed. Show/Hide passphrase is explicit; no phone input unlocks the owner. Storage encryption recovery and owner-screen recovery are separate. Lost owner credentials require authenticated recovery; no hidden bypass.

Connections: `GET /api/settings`, `POST /api/setup`, `GET /api/capabilities`, `GET /api/hardware/audit`; pairing routes from S10; **NEW LOCAL** owner, network, migration, and shell snapshot methods. Boot/data unlock uses initramfs/systemd status, not a network call. Lock status comes from a redacted snapshot.

### S02. Launcher / home desktop

```text
[Home Computer]                 Sun 18:42       [2 devices] [disk] [Wi-Fi] [bell]
                         [ Search apps                         ]
                    [Photos]    [Files]     [Browser]
                    [YouTube]   [Music]     [Notes]
                    [Calculator][Devices]  [Settings]
                         Your files are up to date
                [Home | Photos | Files | Browser | Music | Recents]
```

Hierarchy: app search, nine labelled tiles, quiet status, shelf. App search is local by app name and synonyms in English/Hindi; no implicit file indexing or internet search. Arrow keys move in the grid, Enter opens, Escape clears search then closes Home. Running apps have a dot and accessible “Running” label. Empty search says “No apps found”; disabled hardware features remain explainable inside Settings. Single click/tap launches; launch spinner replaces the icon until ready; failure offers Retry and details.

Connections: `GET /api/overview`, `/api/devices`, `/api/storage`, `/api/alerts`; **NEW LOCAL** shell snapshot/preferences, app launch/focus via compositor registry. No WebView stays alive just to draw Home.

### S03. Photos

```text
[Photos                                             _ □ ×]
[Timeline] [Albums] [Backup] [Duplicates]   [Month v] [Select]
Today           [photo] [photo] [photo] [photo]
Yesterday       [photo] [photo] [photo] [photo]
[Selected: 2]                         [Save a copy] [Move to trash]
```

Hierarchy: memories first; dates/album context second; backup health in a compact strip. Timeline virtualizes rows with cursor pagination and preserves scroll. Albums show existing membership and local create/rename/add/remove controls. Viewer is edge-to-edge with explicit Back, previous/next, zoom, rotate-view, details, and save; Escape exits before closing the app. Rotation is a viewer transformation unless a separate save-copy action is selected. Backup screen shows per-device last verification, pending count and last successful second copy distinctly. A transfer finishing is not proof of a second copy.

Album model: reuse `albums(id,name,created_at)` and `album_items(album_id,file_id,added_at)` with unique `(album_id,file_id)` membership. Album IDs are ULIDs; names are trimmed NFC text, 1–120 Unicode scalar values and ≤480 UTF-8 bytes, with no control characters. Duplicate names are allowed. Local owner authorization applies to all mutations and underlying media; no public album API or new owner column. Create retries reuse a client-generated ID; rename checks the previously displayed name and reports conflicts; add/remove batches of ≤100 file IDs are atomic and idempotent. List pages have ≤100 items with stable date/ID ordering. Local operations are `albums.list/create/update/delete` and `albums.items.list/add/remove`.

Deleting an album removes only its membership, with confirmation “Delete album? Photos stay in your library.” Soft-trashed photos disappear from visible album contents and return on restore; permanently deleted files cascade membership removal. Cover is the first visible member by `(added_at,file_id)`. No manual ordering, custom covers or album trash is implied by v1. Empty album offers Add photos; failed edits retain input; stale rename offers Reload instead of overwriting silently.

States: thumbnail loading, unsupported preview with “Save original,” empty library with “Connect a phone,” offline device with queued status. Duplicate review separates exact duplicates from visually similar photos; compare side by side and choose keepers, then confirm trash. No automatic deletion or phone cleanup. A phone’s existing verified-item/cleanup-lease flow remains the authority for freeing phone storage.

Connections: `GET /api/photos/timeline`, `/api/photos/years`, `/api/files/{id}/thumb`, `/api/files/{id}/content`; `GET /api/duplicates`, `POST /api/duplicates/scan`, `POST /api/duplicates/{id}/resolve`, `GET /api/similar`, `DELETE /api/files/{id}`. **NEW LOCAL** albums and backup summary methods fill the current dashboard gaps; no invented `/api/albums`. Native `save_hub_file` saves originals.

### S04. Files

```text
[Files                                              _ □ ×]
[All files]      [Back] Home / Documents    [Search files] [View]
[Photos]        Name                     Size       Modified
[Videos]        Household.pdf            1.2 MB     Today
[Music]         [selected row]                      [Details >]
[Documents]
[Trash]         [Import a copy]                      [More…]
```

Hierarchy: category/location, searchable content, details on demand. Categories reuse backend filters; search is submitted or debounced 250 ms with cancellation and pagination, never “download everything and filter.” Details show name, type, size, date and verified status. Rename, copy, move, save and trash use explicit menus; drag operations preview destination and copy/move semantics. Trash shows expiry, Restore and Delete permanently with count/name confirmation. Unsupported file types offer Save a copy. Full disk blocks writes while browsing remains available; stale selection is reconciled after refresh.

Connections: `GET /api/files`, `/api/files/{id}/content`, `/api/storage`; `PATCH/DELETE /api/files/{id}`, `POST /api/files/{id}/copy`, `/move`; `GET /api/trash`, `POST /api/trash/{id}/restore`, `DELETE /api/trash/{id}`; `POST /api/import/scan`, `/api/import`, `GET /api/jobs/{id}`; `save_hub_file`. Backend query parameters are taken from current handlers, not new UI-only conventions.

### S05. Browser

```text
[Browser | Recipe × | Weather × | +                  _ □ ×]
[←] [→] [Reload] [ Search or enter website                 ] [☆] [↓]
                    Isolated website content
Downloads: recipe.pdf — Ready                 [Save to Files] [Discard]
```

Hierarchy: recognizable origin/address and navigation, then webpage; bookmarks/downloads are separate drawers. Ctrl+L selects address, Ctrl+T creates, Ctrl+W closes tab. Use a public search provider only after the user submits a search and has accepted internet access. Never silently send typed suggestions. Restore previous tabs is opt-in after a crash. Limit to the existing 12-tab maximum; unload inactive tabs under pressure with a visible reload marker.

States: opening, loading, stopped, offline, certificate error (no bypass), blocked local destination, permission request, download pending/failed/ready. Camera, microphone, location and notifications default denied; explicit grants are origin/session scoped and revoke on expiry or changed origin. Downloading uses a quarantine directory and inert preview; Save to Files requires a separate trusted import/copy confirmation. Never auto-execute downloads.

Connections: existing `browser_open`, `browser_list`, `browser_previous`, `browser_restore`, `browser_action`, `browser_permissions`, `browser_clear`, `save_download`, `discard_download`; bookmark `local_*` commands. These commands remain in the trusted controller, never the remote page. Linux must implement a separate sandboxed web runner with no hub credentials or private-store mount; Tauri capability labels alone are insufficient OS isolation. Internet pages cannot contact hub/LAN/loopback destinations, including through redirects, subresources, DNS rebinding, IPv6 or WebRTC. No hub REST endpoint backs webpage navigation.

### S06. YouTube

```text
[YouTube                              [Site permissions] _ □ ×]
                       Website content
                    [video player / browse]
[Home]                                         [Open in Browser]
```

Hierarchy: site content, clear app identity, reachable Home. Launch a dedicated app-style window and isolated profile using the web runner; this is a PWA-style wrapper, not a promise of offline playback or installable-PWA engine support. Login is optional and stored only in its profile. External links require an explicit Open in Browser transition; fullscreen video hides shelf until pointer reaches bottom or Escape. Media keys target active playback; audio focus pauses another app only after a visible handoff.

States: no internet with Retry/Open Music, sign-in, autoplay blocked, unsupported codec/protected media, profile reset confirmation. Never promise all video resolutions, codecs or DRM services on old hardware.

Connections: existing `youtube_open` and browser permission/lifecycle commands; **NEW LOCAL** audio focus. External HTTPS only; no access to `/api/*`, local files, notes, or device credentials.

### S07. Music

```text
[Music                                              _ □ ×]
[Library] [Playlists]        [Search music] [Sort]
[Road trip]                 Title          Artist      Duration
                            Song one       —           03:42
[Artwork] Song one       [Prev] [Play/Pause] [Next] [Volume] [Queue]
```

Hierarchy: library/playlists, queue, persistent now-playing strip. Queue is reorderable by drag and Move up/down controls. Preserve playback across minimizing and switching apps; close asks whether to stop when playing. Shuffle/repeat expose selected state. Missing metadata uses filename, not invented artist/artwork. Missing files in a playlist remain visible with Remove/Find file; unsupported codec explains the limit. Empty library links to Files import. Scrubbing displays elapsed/total; streaming failure pauses with Retry.

Connections: `GET /api/files` with existing music filtering, `hhmedia` content transport; `local_list/save/trash/restore` for `playlist`; **NEW LOCAL** audio focus and volume; existing `/v1/remote/media` remains the paired-phone route. Playback and queue live in private owner state, not new hub tables.

### S08. Notes

```text
[Notes                                              _ □ ×]
[Search notes] [+ New]       Shopping list          [Saving…]
[Shopping list]             Editable plain text / Markdown
[Weekend ideas]             [Export] [Move to trash]
[Trash]
```

Hierarchy: list/search, title/editor, save status. Reuse existing autosave behavior; debounce 500 ms and flush on blur, app switch, close and lock. Only show Saved after commit. Save failure retains in-memory text, blocks destructive close, and offers Retry/Export. Restore cursor and scroll. Markdown preview disables raw HTML and remote resources. Search is local; no hub-wide sync is implied. Trash distinguishes Restore from permanent deletion; imported notes never overwrite an unrelated note silently.

Connections: `local_list/save/trash/restore/purge` with kind `note`, `note_import`, `note_export`; existing private `desktop.db`. No REST endpoint and no new core schema.

### S09. Calculator

```text
[Calculator                  _ □ ×]
[                      12 × 3    ]
[                           36   ]
[7] [8] [9] [÷]     History >
[4] [5] [6] [×]
[1] [2] [3] [−]
[0] [.] [=] [+]     [Clear]
```

Hierarchy: expression, result, keypad, optional history drawer. Physical number keys work; Enter evaluates, Backspace edits, Escape clears the current expression, second Escape closes the drawer. History item restores expression; Copy result has an accessible confirmation. Invalid expression and divide-by-zero are inline messages; never execute arbitrary code. History is private, bounded to 100 entries, with Clear history confirmation and persistence in an existing `preference` record.

Connections: reuse `desktop/src/calculator.js`; `local_list/save` for history preference. No hub network call.

### S10. Devices

```text
[Devices                                            _ □ ×]
[+ Connect a device]       Connected now (2)
                          Pixel       Backing up   [Manage]
                          MacBook     Ready        [Manage]
                          Previously connected
                          iPhone      Seen yesterday
```

Hierarchy: connect, current activity, known devices. Pair opens a time-limited QR/code panel with expiration and Cancel; requests require local confirmation with device identity and requested permissions. Expired means regenerate, never reuse. Detail shows last seen, permissions, transfer activity and Remove. Removing requires confirmation, revokes certificates and stops access; explain “Files already saved here will stay.” Remote-control permission is separate from file/photo access. Status does not promise wake-from-off.

Connections: `GET /api/devices`, `POST /api/pair/open`, `GET /api/pair/qr`, `/api/pair/pending`, `POST /api/pair/confirm`, `PATCH/DELETE /api/devices/{id}`, `GET /api/transfers`, `/api/remote/status`, `/api/screen/sessions`, `DELETE /api/screen/sessions/{id}`. Phone uses existing `/pair`, mTLS device routes and remote APIs.

### S11. Settings

```text
[Settings                                           _ □ ×]
[Search settings]           Network & internet
[Network & internet]        Home Wi-Fi  Connected    [Change]
[Display & sound]           Nearby networks          [Refresh]
[Files & protection]        [Network name]           [Connect]
[Devices & sharing]
[Appearance & accessibility]
[Security & privacy]
[Updates & recovery]
[About]
```

Hierarchy: eight stable categories, section title, grouped labelled controls. Network covers Wi-Fi/Ethernet/hotspot; display/sound covers scale, brightness, volume, output and lid behavior; files/protection covers space, backup schedules, second copy and disk health; sharing covers permission/session controls; appearance covers theme, wallpaper, language, text, contrast and motion; security covers owner lock, data encryption and internet app permissions; updates/recovery covers signed OS updates, rollback, recovery key and migration; About covers version/hardware and redacted diagnostics. “Power User Mode” exposes detailed diagnostics and supported maintenance tools after owner authentication, not a root terminal or disabled sandbox.

States: edits pending, saving, saved, validation error; radio/toggle failure reverts with a reason. Wi-Fi password never persists in shell preferences. Update stages: available → download → verify → ready → restart → health check → installed/rolled back. Show estimated data migration separately from OS install time. Destructive recovery requires owner authentication and explicit scope.

Connections: `GET/PATCH /api/settings`, `/api/storage`, `/api/second-copy`, `/api/hardware/audit`, `/api/network`, `/api/capabilities`, `/api/diagnostics`; `POST /api/second-copy/run`, `/api/storage/scrub`, `/api/hardware/audit/run`, `/api/pause-sharing`, `/api/network/hotspot`. **NEW LOCAL** display/audio/network/owner/preferences/update operations. Existing Windows `/api/update/stage` is not reused as an OS partition installer.

### S12. Notification center

```text
                                    [Notifications] [Do not disturb]
                                    Today
                                    [!] Storage needs attention [Review]
                                    [✓] Pixel: 24 photos saved    [Open]
                                    Yesterday
                                    [i] Update ready             [View]
                                    [Clear ordinary notifications]
```

Hierarchy: unresolved safety notices, then chronological grouped activity. Panel width 384 px, capped at viewport minus 24; full width on narrow touch screens. Tab stays inside while open, Escape closes, focus returns to bell. Click action opens the owning app with selection/context. Dismiss affects notification state, not underlying failure. “All caught up” is shown only after successful load; failure says “Could not load notifications.” Lock redacts content and actions.

Connections: `GET /api/alerts`, `POST /api/alerts/{id}/ack`, `GET /api/activity`, `/api/transfers`; **NEW LOCAL** notification history and shell snapshot. Shell bridges service events internally; do not assume `/api/events` exists. Store up to 200 redacted notification records in an owner JSON file, outside hub schema, with atomic replacement.

### S13. Quick settings

```text
                                    [Home Wi-Fi      >] [Sharing: On  >]
                                    [Sound          >] [Do not disturb]
                                    Brightness   [--------o--]
                                    Volume       [------o----]
                                    420 GB available · Storage healthy
                                    2 devices connected           [Open]
                                    [Settings] [Lock] [Power]
```

Hierarchy: network/sharing state, display/audio sliders, storage/device summary, power. Tile main area toggles only safe reversible settings; chevron opens detail. Sharing pause warns active transfers before applying. Power menu offers Sleep, Restart, Shut down, with active-job explanation and Wait/Proceed choices; successful completion is never invented during shutdown. Updates and migration inhibit unsafe sleep/restart. No Wi-Fi hardware shows “Use an Ethernet cable”; unsupported brightness removes slider and explains display controls. Storage warning uses icon, label and Review action.

Connections: `GET /api/network`, `/api/storage`, `/api/devices`, `/api/settings`; `POST /api/pause-sharing`; **NEW LOCAL** network, audio, display, power and shell preferences. Network connectivity and internet reachability are separate; internet probe only follows explicit internet-app use or opt-in update checks.

## 7. Physical input, phone remote, ownership and accessibility

The laptop keyboard, touchpad, touchscreen and external mouse use one compositor seat. Two-finger scroll, tap-to-click preference, pointer acceleration and left-handed buttons are configurable; three-finger workspace gestures are optional. Volume/media/brightness keys work while apps are focused, with native non-modal feedback. A USB keyboard remains a recovery path if touch fails.

Phone remote retains `GET /v1/remote/input` WebSocket, `POST /v1/remote/media`, and `POST /v1/remote/power`. Relative pointer, click, scroll, key and text semantics remain unchanged. Linux helper translates through uinput and the compositor text-input path for Unicode. Never simulate arbitrary Unicode via a US keyboard layout. Local input takes priority; remote control displays a persistent “Controlled by [device]” chip and Stop action. Block remote input, capture, pairing approval, credential entry and destructive recovery while locked or in a secure local prompt. Disconnect releases every held key/button. Cast/view support is capability-reported and revoked on lock.

Sleep shows “Pausing transfers…” until a durable checkpoint is reached; active installation/migration explains why sleep is unavailable. Lock and revoke remote/capture grants before suspend. On wake keep the owner screen locked, show “Reconnecting…” rather than “Ready,” recheck storage/network/time/revocation state, and establish fresh mTLS sessions. Transfers reconcile saved chunk status before resuming; an uncertain completion is queried rather than replayed blindly. An expired/revoked certificate must fail even if its connection worked before sleep. Missing network says “Waiting for connection”; do not ask the user to re-pair for ordinary Wi-Fi recovery. Remote control/capture require new consent. Time errors are explained and never bypassed. Same-hub sleep preserves keys and pairing identity.

Single owner v1, with explicit owner UUID on app launch, preferences, notifications and private-store paths. Connected device names identify senders, not additional OS users. Hide a user switcher until multi-user authorization/storage actually exists. Borrowed-phone pairing uses restrictive scopes; no implicit access to owner notes.

Accessibility is a P4.2/P4.3 release gate: native chrome exposes roles, names, values, focus and actions through an AT-SPI bridge; Svelte content uses semantic HTML and labelled controls. Boot recovery, OOBE, lock and on-screen keyboard must work with the chosen screen reader. Drawing text with a GPU does not create accessibility semantics. Test keyboard-only navigation, Orca, 200% text/scale, high contrast, reduced motion, Hindi composition, and loss of pointer. Announce transfer milestones rather than every percentage change. Preserve focus after dialogs, removal and failed operations; communicate validation next to its field and in the accessible status region.

## 8. Designer and implementation acceptance

Produce frames for S01–S13 at 1366×768 mouse, 1920×1080 mouse, and 1280×800 touch, plus a 200% scale variant. Include dark/light/high-contrast tokens; hover/focus/pressed/disabled/loading/empty/offline/error states; first run, returning user, disk-full, locked and unsupported-hardware flows. Use original app artwork and locally licensed assets. Validate the first-use tasks with non-technical English and Hindi readers: open photos, find a document, pair a phone, adjust sound, recover a failed save, and return Home without instructions.

Performance targets remain boot <15 seconds, total idle resident memory <200 MB, idle CPU <3%; they are unmeasured requirements, not claims. Launcher idle must have no resident WebView, video decoder, capture pipeline, animated wallpaper or repeated whole-screen redraw. Active browsing/media are measured separately and must never threaten transfers or saved data. If accessibility or native chrome cannot meet the budget, return the measured tradeoff for a product decision; do not omit accessibility or silently redefine idle.

## 9. Evidence and technical references

Repository evidence: `docs/UI_UX_DESIGN.md` §§2, 9–12; `docs/PRD.md` §§5, 9; `docs/TRD.md` §§15–16; actual routes in `hub/crates/hh-net/src/{dashboard,admin,routes}.rs`; trusted commands in `desktop/src-tauri/src/main.rs`; private storage in `desktop/src-tauri/src/store.rs`; current views in `desktop/src/App.svelte`. Source was checked where the codebase graph reported stale metadata. Proposed local methods above are additions, not existing route claims.

The security boundary follows the separation between trusted native code and WebView content described in [Tauri security](https://v2.tauri.app/security/). The baseline follows [labwc's compositor/client split](https://labwc.github.io/integration.html); optional [Smithay](https://github.com/Smithay/smithay) is a compositor library rather than a complete desktop. Both require platform/isolation validation. Budget 32–40 solo-developer weeks for the labwc plan; custom-compositor work is separately estimated and cannot be inferred from these wireframes.
