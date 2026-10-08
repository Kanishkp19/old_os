# Home Hub — UI/UX Design

Windows product scope includes the Home Hub launcher plus Browser, YouTube, Music, Notes and Calculator. Settings exposes screen permission, backup and protection intervals, optional scheduled wake, and a signed update channel. The device must show the actual helper, disk and wake capabilities: unsupported hardware is labelled unavailable or unknown. Private app data stays in the Windows owner's profile; imported original files remain in place unless the owner explicitly selects a verified copy or Keep in place. Phone cleanup shows a fresh, pinned Hub copy for every eligible item before Android's system confirmation.

**Surfaces:** Android app · macOS app · Windows tray + local dashboard · (iOS Phase 2) · (Home Hub OS shell Phase 4)
**Mental model:** *"This is my Home Computer."* Never "server", "NAS", "SMB", "IP address".

---

## 1. Design principles

1. **Say what you want to do, not which technology.** Actions: *Send to Home, Back up photos, Open my files, View home computer.*
2. **One tap to the common thing.** Send and Back up are always reachable from the first screen.
3. **Trust through visibility.** Always show: verified ✓, how many copies exist, drive health.
4. **Honest status.** Distinguish *No internet* vs *No home network* vs *Hub asleep/off*. Never a generic "error".
5. **Queue, don't fail.** Offline → "Queued, will send when Home is back."
6. **Calm and light.** Fast, low-motion, works on old laptops and mid-range phones.
7. **Plain language, optional detail.** Advanced info behind "Details" and Power User Mode.
8. **No dark patterns around deletion.** "Free phone storage" is explicit, previewed, and gated on verification.

## 2. Visual language

### Tokens
| Token | Light | Dark | Use |
|---|---|---|---|
| `--bg` | #FAFAF7 | #121413 | App background |
| `--surface` | #FFFFFF | #1B1E1C | Cards |
| `--ink` | #1A1D1B | #ECEFEA | Primary text |
| `--ink-2` | #5B635E | #A3ABA5 | Secondary text |
| `--accent` | #2F6F4E | #5FBF8F | Primary actions, online |
| `--accent-ink` | #FFFFFF | #0B1A12 | Text on accent |
| `--warn` | #B7791F | #E5B05C | Caution (limited, queued) |
| `--danger` | #B3261E | #F2877F | Failing drive, errors |
| `--ok` | #2E7D32 | #6CC070 | Verified |
| `--line` | #E3E6E1 | #2A2E2B | Dividers |

Color is never the only signal; always pair with icon + text.

### Type
- UI: system sans (Roboto / SF / Segoe UI Variable) — no web fonts on resident components.
- Scale: 12 / 14 / 16 / 20 / 28 / 36. Body 16. Line height 1.4.
- Numbers (sizes, speeds) tabular.

### Shape and spacing
- 8-pt grid. Radius: 12 (cards), 999 (pills), 8 (inputs). Elevation: minimal (1px line + soft shadow on modals).
- Touch targets ≥ 48 dp. Desktop ≥ 32 px.

### Iconography
Simple outline set (Lucide/Material Symbols). Home Hub glyph: house + signal arcs. Emoji only in marketing/mockups, not production UI.

### Motion
150–200 ms ease-out. Respect reduced-motion. Progress bars are determinate whenever size is known.

## 3. Information architecture

```text
Phone / Mac app
├── Home (dashboard)
│   ├── Send to Home
│   ├── Back up photos
│   ├── Storage summary
│   └── Alerts
├── Photos         (Timeline → Year → Month; Duplicates)
├── Files          (Photos · Videos · Documents · Music · Backups · Downloads)
├── Transfers      (Active · Queued · History)
├── Remote         (Trackpad · Keyboard · Media · Power)     [scope: remote]
├── Devices        (This device · Other devices · Hub info)
└── Settings       (Backup rules · Notifications · Privacy · About)

Windows tray + dashboard (loopback)
├── Status          (Online · Free space · Devices · Transfers)
├── Pair a device   (QR)
├── Storage         (Library · Health · Second copy · Import existing files)
├── Devices
├── Hardware        (Audit · Recommended features)
└── Settings        (Library location · Network · Updates · Advanced)
```

## 4. Key flows

### 4.1 First-time setup (Windows)
1. Installer finishes → tray icon + welcome window: **"Set up your Home Hub"**.
2. Name it (prefilled "<User>'s Home Hub").
3. Choose where files live (recommended drive shown with free space; warning if only C: and <50 GB free).
4. Hardware check (5 s animation) → summary: "Great for storage and photo backup. Wi-Fi is slow: use Ethernet for best speed."
5. Existing files found → Keep / Import / Later per category (default **Later**).
6. QR screen: "Connect your phone".

```text
┌──────────────────────────────────────────────┐
│  Connect your phone                          │
│                                              │
│            ┌──────────────┐                  │
│            │   QR CODE    │   1. Install Home Hub on your phone │
│            │              │   2. Open it and tap "Connect"       │
│            └──────────────┘   3. Scan this code                 │
│                                                                  │
│  Code expires in 4:52          [ Use a code instead ]            │
└──────────────────────────────────────────────┘
```

### 4.2 Pair (phone)
```text
Welcome → [Connect to my Home] → Camera opens (permission with plain-language reason)
→ Scan → "Connecting…" → ✓ "Connected to Kanishk's Home Hub"
→ "Back up your photos?" [Not now] [Choose…]
```
Target: <30 s. If scan fails: nearby Hubs list (mDNS) → "Enter code".

### 4.3 Send a file
Share sheet → **Home Hub → My Home** → bottom sheet with progress, ETA, ✓ verified.

```text
Sending to My Home
IMG_2041.MOV        2.0 GB
████████░░░░░░░░  48%   about 1 min left
[Cancel]
```
Complete: "✓ Saved to My Home · Videos" [Open] [Done]

### 4.4 Hub unavailable → queue
```text
⏳ Home Hub isn't reachable right now.
Your file is queued and will send automatically when Home is back.
[View queue]
```
Notification on completion: "2 files sent to My Home ✓".

### 4.5 Photo backup
```text
Back up your photos
487 new photos · 63 videos   (~6.2 GB)
Sent over your home Wi-Fi only. Nothing leaves your home.
[Back up now]   [Choose what to back up]
```
Progress → "487 of 550 · Verified ✓". After completion:
```text
✓ 550 items safely backed up
Free up 6.2 GB on this phone?
We only remove items confirmed on your Home Hub.
[Review & free space]  [Not now]
```
Review screen lists count/size, shows "Verified" badge, requires explicit confirm via system delete dialog.

### 4.6 Disk warning
```text
⚠ Your Home Hub's drive may be failing
Back up important photos to a second drive now.
[Set up second copy]  [Remind me tomorrow]
```
Danger state persists on Home until resolved. No "dismiss forever".

### 4.7 Second copy nudge
Persistent Home card when only one copy exists: **"1 copy only. Add an external drive for a safe second copy."** (Warn tone, not alarming; escalates in severity if disk health degrades.)

### 4.8 Remote control
```text
MY HOME COMPUTER
[ Trackpad area (large) ]
[⌨ Keyboard] [🎵 Media] [⏻ Power]
```
- Trackpad: 1 finger move, tap click, 2-finger scroll, long-press right-click.
- Power: confirm dialog "Shut down Home Hub? You can't turn it on from your phone unless Wake is set up."
- Banner on Hub: "Remote active: Kanishk's Pixel".

### 4.9 Remove device
Devices → select → **Remove device** → confirm → immediate revoke; device sees "This phone was disconnected from My Home."

### 4.10 Screen sharing (Phase 3)
Remote → **View on phone** / **Cast to laptop**; quality auto-chosen from hardware audit; "Limited on this laptop" message when audit says so.

## 5. Screens (Phone)

### Home
```text
┌──────────────────────────────┐
│ My Home            ● Online  │
│ Kanishk's Home Hub · Wi-Fi   │
├──────────────────────────────┤
│ [ 📤 Send to Home ]          │
│ [ 📷 Back up photos ]        │
├──────────────────────────────┤
│ Storage  382 GB free of 465  │
│ ▓▓▓▓▓▓░░░░                   │
│ Backups  Last: today 9:14 ✓  │
│ Copies   1 (add second copy) │
├──────────────────────────────┤
│ Alerts (0)                   │
└──────────────────────────────┘
 Home · Photos · Files · Remote · More
```

### Connection states (header chip)
| State | Chip | Behavior |
|---|---|---|
| Connected (LAN) | ● Online | Normal |
| Hub not found | ◐ Looking for Home… | Queue enabled, retry via mDNS |
| Hub asleep (WoL supported) | ◐ Waking Home… | Send magic packet, wait 30 s |
| Hub asleep/off (no WoL) | ○ Home is off or asleep | Queue; tip: "Turn on your Hub" |
| No Wi-Fi | ○ Not on home Wi-Fi | Queue; show Wi-Fi hint |
| Internet down, LAN OK | ● Online (no internet) | Subtle note: "Everything works without internet" |

### Photos
- Grid, grouped by Year → Month, sticky headers, fast scrub bar.
- Badges: ✓ on Hub, ☁︎-less (no cloud icons). Duplicate banner: "2,314 possible duplicates · recover 17.8 GB".
- Viewer: pinch zoom, share, delete (to Hub trash), details (date, size, hash verified).

### Files
Category tiles first (Photos, Videos, Documents, Music, Backups, Downloads) → list view with sort/search. Never show Windows paths.

### Transfers
Tabs: **Active · Queued · History**. Each row: name, size, state, ETA, retry/cancel. Failed rows state the reason plainly ("Home was turned off") with a Retry.

### Devices
This device (rename), others (last seen, platform, Remove), Hub info (name, version, network mode, Wi-Fi standard, "Add another device" QR).

### Settings (phone)
Backup rules (Wi-Fi only default on, charging only default off), Notifications, Privacy (LAN-only statement, location metadata toggle), Advanced → "Power User Mode" (shows IPs, ports, logs), About.

## 6. Screens (Windows tray + dashboard)

### Tray popup
```text
Home Hub
● Online
Storage      382 GB free
Devices      2 connected
Transfers    None

[Open Home Hub]  [Pair a device]  [Pause sharing]
```
Tray icon states: green (ok), amber (attention: no second copy, low space), red (critical: failing drive), grey (paused).

### Dashboard (localhost:47801)
Left nav: Overview · Devices · Storage · Photos · Hardware · Settings.
- **Overview:** Status card, active transfers, alerts, quick pair.
- **Storage:** Usage by category, health per drive (Good / Caution / Failing), last integrity scrub, second-copy age, Import existing files wizard.
- **Hardware:** Audit with per-capability rating chips (Excellent / Good / Limited / Not recommended) and plain-language explanation.
- **Settings:** Library location (with move wizard), Network mode, Auto-start, Updates (toggle), Advanced.

## 7. macOS app
Menu bar item with drag-and-drop target ("Drop files to send to Home"), Finder Share extension, window with Home/Files/Transfers. Sync folder is **out of scope** for MVP.

## 8. Home Hub OS shell (Phase 4 preview)
Full-screen launcher grid: Photos, Files, Browser, YouTube (PWA), Music, Notes, Calculator, Devices, Settings. Large touch/trackpad friendly targets. Power User Mode reveals terminal/system info.

## 9. Content and microcopy

| Situation | Copy |
|---|---|
| Name of Hub | "My Home" (user-facing); "Home Hub" (brand) |
| Success | "Safely saved to My Home ✓" |
| Verifying | "Checking every byte…" → "Verified ✓" |
| Queued | "Waiting for Home. We'll send this automatically." |
| No internet | "No internet. Everything on your home network still works." |
| No LAN | "You're not on your home Wi-Fi." |
| Hub off | "Home Hub is turned off or asleep." |
| Low storage | "Home is almost full (6% free). Free up space or add a drive." |
| Failing drive | "Your Home Hub's drive may be failing. Back up important photos now." |
| Single copy | "1 copy only. A second copy keeps your photos safe." |
| Delete confirm | "Remove 550 items from this phone? They're verified on your Home Hub." |
| Remove device | "This phone will lose access immediately." |

Rules: second person, short, no jargon (avoid: sync, daemon, hash, mTLS, SMB, port). Hindi translation planned; keep strings externalized and avoid text-in-images.

## 10. Accessibility
- WCAG AA contrast, 4.5:1 body.
- Dynamic type to 200%; layouts reflow.
- TalkBack/VoiceOver labels on all controls; progress announced at 25% steps.
- Trackpad alternative: D-pad/arrow controls and on-screen click buttons.
- Not color-only: state chips carry icon + word.
- Reduce motion respected.

## 11. Permissions UX
Ask in context, with a reason line:
- Camera (QR): "To scan the code on your computer."
- Photos/Media: "So Home can back up your photos. Only over your home Wi-Fi."
- Local network (iOS/Android 13+ nearby devices): "To find your Home Hub on your Wi-Fi."
- Notifications: "To tell you when backups finish or Home needs attention."
- Battery optimization exemption (Android): only prompted if backups are being killed; explain why.

## 12. Empty, loading, error states
- **Empty Photos:** "No photos yet. Back up your phone to see them here." [Back up now]
- **Loading:** skeleton grid, never blocking spinners for lists.
- **Errors:** what happened + what we're doing + one action. Example: "Couldn't reach Home. We'll keep trying. [View queue]"

## 13. Design deliverables checklist
- [ ] Figma: tokens, components (chip, card, progress row, list row, bottom sheet, banner)
- [ ] Flows: setup, pair, send, queue, backup, free space, disk warning, remote
- [ ] Android + macOS + dashboard high-fidelity screens
- [ ] Dark mode
- [ ] Hindi layout stress test
- [ ] Usability test with 5 non-technical users before Phase 1 exit
