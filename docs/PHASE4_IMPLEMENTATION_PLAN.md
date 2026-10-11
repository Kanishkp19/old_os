# Phase 4 — Home Hub OS Implementation Plan

Status: proposed plan, 11 October 2026. Deliver a bootable Linux appliance with native OS chrome, the existing Rust backend, and reused Svelte app views. This is a documentation deliverable, not an implemented OS, a new dependency installation, or a claim of hardware acceptance. The user's request authorizes this planning work; the existing completed E2E milestone remains unchanged.

## 1. Baseline, invariants and decisions

The source has 14 backend workspace crates, Tauri 2/Svelte desktop apps, a fixed authenticated loopback proxy, and **13** hub migration files through `0013_scrub_rotation.sql`. Preserve all 13 and their tables; do not roll back to the prompt's historical count of 11. The private desktop SQLite store already contains records and browser-session tables. Use those for notes/playlists/bookmarks/preferences, and existing album tables for albums. Add no core tables for the OS.

The graph was indexed on 8 October 2026; coverage checks reported changed metadata in configuration, routes, service and desktop sources. Relevant source ranges and SQL gaps were inspected directly. Paths in change maps below are repository-relative; **EDIT** identifies an existing file, **NEW** a proposed file. Line counts are rough implementation sizing, exclude generated code, and are not commitments.

Hard invariants:

- Preserve mTLS on LAN 47800; authenticated loopback dashboard on 127.0.0.1:47801; time-boxed pairing on 47802; mDNS 5353 and discovery UDP 47803. No new public REST route or TCP listener for the OS.
- Preserve verification → fsync → same-filesystem atomic rename → database commit and restart recovery. Linux support must not weaken path jails, revocation, transfer hashes, cleanup leases or second-copy accounting.
- Retain Windows implementations/builds. Exclude the Windows tray from the Linux image, not from the Windows product. Linux helper implementation coexists with Windows Session 0 code.
- Single owner v1; owner IDs scope private state. Read-only system root; signed image updates; no root login, resident Electron, default telemetry or unsolicited outbound requests. Browser/YouTube traffic occurs only after explicit use; update checks remain opt-in.
- Native Home must meet boot <15 s, whole-system idle memory <200 MB and idle CPU <3%. Retain the tighter existing service budget <80 MB/<1% CPU. These are release gates with measurements, not architecture guarantees.

### 1.1 Chosen product structure

Use **Option C**, with **labwc plus a native Rust shell as the delivery baseline**, and on-demand Tauri/WebKitGTK content apps. Smithay is an optional custom-compositor track, not a three-week production task. Budget 12–24 weeks (approximately 3–6 months) for that track as an engineering planning allowance, subject to evidence; a Week 1 demo cannot prove production readiness. Buildroot + glibc + systemd is a candidate production base until the Week 1 VM dependency spike passes; Debian/Ubuntu remains the development host. GRUB/RAUC, NetworkManager and PipeWire/WirePlumber remain the planned boot/update/network/media components. No new dependencies are installed by this documentation revision.

The UI is Linux-inspired overview/workspaces, Windows-inspired shelf and snapping, macOS-inspired quiet materials, and Android-inspired quick settings/touch navigation. This resolves to one native top status bar, one bottom shelf, right-hand window controls, nine launchable apps, full-work-area touch mode, optional floating/split mouse mode, owner lock, notification center, and quick settings.

## 2. Linux base system

### 2.1 Distribution comparison

| Candidate | Advantages for this product | Costs / limitations | Decision |
|---|---|---|---|
| Buildroot | Reproducible whole images; explicit package/firmware set; good fit for A/B and minimal resident system | Integrating current GTK/WebKitGTK/Rust toolchain and security rebuilds is substantial; no end-user package manager | Candidate only; mandatory Week 1 dependency-chain gate |
| Yocto | Layered board support, reproducible policy and image tooling | More metadata/build expertise and maintenance than this team's current stack | Reconsider for multiple hardware SKUs after v1 |
| Debian minimal | Fast hardware bring-up, familiar debugging, mature desktop dependencies | Needs image discipline/read-only packaging; background packages must be removed; budget still unproven | Development VM and contingency prototype, not an unannounced production switch |
| Alpine | Small userspace and package set | musl/OpenRC integration differs from selected glibc/systemd stack; WebView/native compatibility work | Do not choose for v1 |

Buildroot supports systemd integration; exact configuration depends on the pinned build. Select glibc/systemd deliberately rather than assuming the smallest default configuration has desktop services. [Buildroot manual](https://buildroot.org/downloads/manual/manual.html).

Before committing to Buildroot or a custom compositor, complete P4.S in §2.3.1: build and run a real Tauri/WebKitGTK app inside the generated image in QEMU. A successful desktop-Linux build is only diagnostic evidence. Pin Buildroot release/commit, Linux LTS revision, firmware hashes, Rust toolchain, all Cargo/npm locks and package sources in the image manifest. P4.1 later repeats the accepted chain on hardware; neither stage proves arbitrary laptop compatibility.

### 2.2 Kernel and userspace requirements

| Area | Required configuration / package families | Validation |
|---|---|---|
| Boot/storage | EFI/GPT, BIOS boot support where tested, initramfs, SATA/AHCI, NVMe, USB storage, ext4, squashfs, dm-verity, device mapper, dm-crypt, FAT ESP | Boot SSD/USB, mount data, reject corrupt root, power-loss recovery |
| Graphics | DRM/KMS, Intel i915, AMDGPU, Nouveau only for validated models, framebuffer console recovery; Mesa/EGL/GLES | Modeset, cursor, acceleration, reconnect, suspend/resume, fractional scale |
| Input | evdev, HID USB/I2C, PS/2, multitouch, uinput; libinput, xkbcommon, input-method support | Physical and virtual input, key release on disconnect, Hindi composition |
| Network | cfg80211/mac80211, Ethernet, validated Wi-Fi drivers and distributable firmware, rfkill; NetworkManager, DHCP/DNS support | Reconnect, credentials, offline LAN, no unexpected inbound exposure |
| Audio/video | ALSA/HDA/SOF/USB audio as needed; PipeWire/WirePlumber; validated VA-API encoders | Output switch, media keys, capture stop on lock, software fallback bounds |
| Security/runtime | namespaces, seccomp, cgroups, UNIX sockets, nftables, RNG, TPM2 optional; systemd/logind, D-Bus, polkit, LUKS tools | Unprivileged app isolation, permitted helper operations, no root login |
| Hardware reporting | sysfs/udev, smartmontools, privileged fixed-purpose probe for DMI when required | SATA/NVMe/USB unsupported cases produce “unknown,” not healthy |

Build firmware inclusion from the supported hardware matrix; record redistribution licenses. Secure Boot support requires a supported signing/enrollment chain for bootloader, kernel and recovery; an image signature alone does not establish Secure Boot. Unsupported GPUs/Wi-Fi receive a compatibility explanation before installation.

### 2.3 Compositor comparison and decision

| Choice | Fit | Decision |
|---|---|---|
| cage | Small single-app kiosk surface model | Good smoke-test harness; inadequate for full window/chrome policy |
| labwc | Existing wlroots stacking compositor and desktop protocols | Delivery baseline and mandatory Smithay fallback; separate native shell clients and file map in §5.4 |
| sway in kiosk configuration | Mature tiling and bindings; “sway-kiosk” is a configuration approach here | Poor default floating/touch product fit; useful diagnostic environment |
| Custom wlroots compositor | Full scene/policy control | Adds C/FFI/version integration to Rust codebase |
| Custom Smithay compositor | Rust composition with direct native chrome and controlled input/capture policy | Optional 12–24-week track only after Week 1 gate and a separately resourced decision |

Smithay supplies compositor building blocks, not a ready-made shell. `hh-compositor` must implement scene management, xdg-shell, popups, seat/focus, output scaling, clipboard restrictions, text input/IME, secure lock, accessibility integration, and capture authorization. Start nested for development, then DRM/KMS/logind. Do not combine Smithay and wlroots into an undefined “surface API.” [Smithay upstream](https://github.com/Smithay/smithay). labwc is an established wlroots stacking compositor and remains the named fallback if the custom-compositor gate fails. [labwc manual](https://labwc.github.io/labwc.1.html).

Do not begin production Smithay modules on the assumption that a three-week spike will produce a safe desktop. labwc owns the compositor in the baseline; `hh-shell` owns product chrome. The optional `hh-compositor` file map is retained for a deliberate later branch, not work to perform alongside the baseline.

### 2.3.1 P4.S — hard Week 1 feasibility gate

Timebox: five working days, before P4.0. Scope is a disposable VM image and a minimal vertical slice, not a full shell. Start long dependency builds on day 1. Record build failures and missing packages rather than extending the spike indefinitely; observe the command retry limit.

| Day | Required experiment | Evidence |
|---|---|---|
| 1 | Pin exact Rust/Buildroot/GTK/WebKitGTK/Wayland/compositor dependencies; build minimal packaged Svelte/Tauri content and start image build | Lockfiles, toolchain/MSRV and dependency inventory, build log, reproducible commands |
| 2 | Boot generated Buildroot image in x86_64 QEMU with labwc; launch that packaged app and connect to the fixed local hub proxy | Image hash, serial log, screenshot, one authenticated local read; no host libraries mounted into guest |
| 3 | Run the same app/image with a minimal Smithay harness; exercise keyboard, Hindi composition, clipboard, popup/focus, resize/scale and close/reopen | Per-case pass/fail and crash logs; explicitly distinguish nested from guest DRM runs |
| 4 | Test native Rust/GTK layer-shell panel on labwc, AT-SPI control discovery, lock/capture protocol availability, guest network/audio and idle memory; restart twice | Recorded protocol inventory, screen-reader smoke, memory/process report; software-renderer limits disclosed |
| 5 | Rebuild from saved inputs and repeat boot/app/proxy smoke; issue written go/no-go with blockers and selected stack | `results.json`, dependency lock, image digest and `DECISION.md`; no unchecked box counts as pass |

**Decision rules:** Buildroot+labwc+Tauri must run inside the image to accept that base. Smithay failure, missing evidence or no engineer funded for 12–24 weeks selects labwc and stops custom-compositor work. Smithay success permits a separately estimated track; it does not change the baseline automatically. If labwc works but Buildroot packaging fails, stop base-image commitment, run a separately approved Debian-minimal image spike, and re-estimate packaging/security updates. If neither compositor chain works, stop dependent shell/app scheduling. Do not infer labwc success from Smithay failure. A missing VM/toolchain is a blocked gate, not approval to skip it.

The minimum screen-reader/lock protocol smoke is necessary but not a full safety certification. P4.2/P4.3 still require focus/IME/accessibility, compositor crash/lock and capture denial tests. A Week 1 budget failure opens an explicit resource decision; it is not resolved by omitting GTK/accessibility from measurements.

New spike files (not yet implemented): `tools/image-builder/spikes/wayland-webkit/{README.md,build.sh,run-qemu.sh,smoke.sh,dependencies.lock.json,results.json,DECISION.md}`, `smithay-harness/{Cargo.toml,src/main.rs}`, and `tauri-smoke/{package.json,src/App.svelte,src-tauri/Cargo.toml,src-tauri/tauri.conf.json,src-tauri/src/main.rs}` under that spike directory. `build.sh` packages the test image; `smoke.sh` records the matrix for both compositor choices. Keep image/log binaries outside Git with hashes in results. These are the entire first implementation slice; production file changes wait for the decision.

### 2.4 Boot and filesystem layout

BIOS/UEFI → GRUB verified slot selection → kernel/initramfs → verify root image → unlock/mount data → systemd → service/helper and compositor → native OOBE or lock → Home. Network-online must not block the launcher; pairing and network features become ready independently. “Interactive” means keyboard/touch works and local file service is ready, not merely a displayed splash.

| Partition / mount | Initial size | Contents / rules |
|---|---|---|
| BIOS boot partition | 2 MiB when needed | GRUB BIOS embedding; omitted for UEFI-only supported image |
| ESP | 512 MiB FAT32 | Bootloader, slot-specific signed boot assets, boot state; never format an existing Windows ESP in Keep Windows mode |
| root A / root B | 6 GiB each | Read-only system image plus integrity metadata; size gate checks WebKitGTK/firmware fit with 20% headroom |
| Recovery | 2 GiB | Signed minimal recovery environment; validation tools and restore UI |
| Data | Remaining space, minimum 16 GiB plus migrated data and headroom | ext4, optionally inside LUKS2; mounted by stable UUID, not `/dev/sda` assumptions |

At least 32 GiB available install space for OS plus minimal data is the planning floor; installer computes actual image and migration requirements. HDD, larger firmware bundles and encryption can change time and size requirements. Do not proceed when headroom is insufficient.

`/etc/homehub` holds image defaults. Writable configuration is `/var/lib/homehub/config.json` on Data; do not write generated settings into read-only `/etc`. Service state, identity and `hub.db` live in `/var/lib/homehub`. Library root is `/srv/homehub`, backed by Data; retain existing derived `library`, `.hh-tmp`, thumbnails and trash semantics. Partial files and final destinations stay on that same filesystem. `/var/lib/homehub-users/<owner-id>` holds private desktop state. `/run/homehub` and `/run/user/<uid>` hold ephemeral sockets/tokens; bounded logs persist on Data. Use restrictive service-owned and owner-owned directories, not a shared world-readable tree.

### 2.5 A/B update protocol

Download only with opt-in; verify signed bundle and board/product compatibility; reject unauthorized downgrade before writing; write and verify inactive slot; preserve Data; set pending slot with three boot attempts; reboot when user accepts. A local health service marks good only after mounted data, DB readability, service readiness, input-capable shell and 60 seconds without a critical crash. Internet availability is not a health criterion. Exhausted attempts return to the last good slot and display the reason.

RAUC installation and boot confirmation are separate operations; successful writing alone does not mean a working boot. Integrate bootloader state explicitly. [RAUC basics](https://rauc.readthedocs.io/en/latest/basic.html).

Use a separate OS update signing trust root from the existing Windows installer updater. Root signatures, rollback counters and trust rotation are part of release tooling. Ordinary root-slot updates do not rewrite shared bootloader/recovery assets; update those only with a recoverable separately verified process. New/old slots must read shared data/config both ways; Phase 4 adds no core schema migration. Version private JSON formats and keep previous readable copies. If a future DB change breaks backward compatibility, block routine rollback and require an explicit tested recovery policy before shipping it. Never “rollback” by replacing shared Data with an old snapshot and losing new files.

## 3. Rust service porting — every existing crate

| Crate | Reuse | Linux work / exact responsibility |
|---|---|---|
| `hh-core` | Types, errors, hashing identifiers and public traits | Linux production paths; owner/session types; preserve actual `InputControl` methods including scroll/text/media and `ServiceHost::run(self: Box<Self>)` |
| `hh-db` | SQLite/WAL, queries, all 13 migrations | No schema changes; backup snapshot/read-only migration checks using existing SQLite facilities; existing album query support as local service operations |
| `hh-auth` | CA, mTLS, pairing, revocation and renewal | Harden Unix key creation/ownership; encrypted migration export/import; Linux currently writes PEM with mode 0600, which is not encryption |
| `hh-net` | Existing URLs, payloads, ports, middleware, TLS, discovery | Wire Linux providers into current handlers and capability reporting; internal local broker for albums/summary/snapshots; Linux update operations separate from Windows installer staging |
| `hh-transfer` | Chunk/resume/BLAKE3 engine and reconciliation | No planned algorithm changes; verify Linux fsync/rename/disk-full behavior and migrated partials |
| `hh-storage` | Library, jobs, dedupe, trash, second copy and journals | Replace only platform-specific volume/path/probe assumptions; bind UUID-based root mapping; migration copy verifier |
| `hh-photos` | EXIF, thumbnails, backup proof and gallery | No format rewrite; expose existing album/backup logic to local broker, preserve Linux codec limitations |
| `hh-remote` | Scope/session rules and helper request types | Linux uinput adapter and logind power adapter; protected socket transport; no service-wide unrestricted input-device access |
| `hh-hw` | Rating logic, report DTOs and compatibility model | sysfs/udev/DMI audit, SMART/NVMe JSON parsing, NetworkManager adapter; mark unavailable probes explicitly |
| `hh-stream` | Session authorization, limits and signalling | Linux capture capability and PipeWire stream lifecycle; real compositor-produced frames required, not just linking PipeWire |
| `hh-service` | AppState, listeners, database lifecycle and jobs | systemd host/signals/readiness/watchdog, Linux dependency wiring, lock-aware policy, no identity auto-generation on incomplete import |
| `hh-session` | Protocol, dispatch and existing Unix socket transport | Add Linux dispatch/input/capture/backend; peer credentials and session ownership; retain Windows implementation |
| `hh-tray` | Existing Windows tray | Leave Windows source intact; omit Linux build/image installation; shell consumes status instead |
| `hh-tools` | Diagnostics, fake client and benchmarks | Migration export/verify/import, image/idle measurement and Linux-specific diagnostic probes |

### 3.1 Input, power, hardware and capture details

uinput creates virtual input devices; it does not authenticate callers or implement Unicode text policy. The helper creates fixed virtual keyboard/pointer devices, maps each existing protocol operation, clamps deltas/rates, releases pressed keys on disconnect and lock, and tags remote events so secure surfaces reject them. Unicode text uses compositor-mediated text input for the focused non-secure surface; no arbitrary clipboard overwrite and no US-layout shortcut. Unknown keys return the existing unsupported/error shape. [Linux input userspace API](https://kernel.org/doc/html/latest/input/input_uapi.html).

Power uses logind with a narrow policy for the active authorized session. Installation/update/export take inhibitor locks; explicit shutdown waits for flush/reconciliation boundaries. Hardware probes parse machine-readable output and never compose a shell command with device/user input. Resolve disks through enumerated stable IDs; privileged probing accepts only an allowlisted device descriptor and bounded timeout/output.

Capture path: compositor checks active grant → exports authorized output frames to a restricted PipeWire node → session helper consumes frames → encoder → existing WebRTC path. Implement the producer and permission broker; PipeWire alone cannot capture a custom desktop. DRM render nodes accelerate encode where supported; unprivileged helper never grabs the DRM master display behind the compositor. Only the compositor uses DRM/KMS for scanout. Software encoding is bounded and capability-reported. Lock and revocation tear down the PipeWire node and WebRTC stream before private content could leak. Default preset is 720p/30 only where measured; unsupported hardware exposes `available:false` rather than fabricated capability.

### 3.2 Linux identity protection

Fix current Unix key creation to create mode 0600 atomically, reject symlinks/unexpected ownership, fsync file and parent, and fail on partial identity sets. Permissions protect against other local accounts; do not call them encrypted. Production data protection uses optional LUKS2: passphrase unlock for machines without a validated TPM flow, or TPM2-sealed unlock with recovery key on tested hardware. A Linux keyring is runtime custody, not durable reboot-safe storage. A wrapping key beside encrypted files on the same unencrypted disk does not provide theft protection.

Import DPAPI-protected keys only through an encrypted bundle created on the original Windows installation. Do not copy the DPAPI blobs and expect Linux to decrypt them. Preserve CA bytes/fingerprint, hub ID, valid device certificates/scopes/revocations and renewal state. Clients retain their private keys. A moved machine may require a new server leaf certificate signed by the same CA; verify every companion's pinning behavior before declaring seamless migration.

### 3.3 Suspend/resume and mTLS connection lifecycle

Implement `Awake → PreparingSleep → Suspended → Recovering → Awake/Offline` in new `hub/crates/hh-service/src/power.rs`. Subscribe to logind sleep notifications; acquire a bounded delay inhibitor while quiescing. UI-requested sleep waits for a safe checkpoint or shows why it cannot proceed. Firmware/forced sleep may exceed application control, so restart reconciliation must also tolerate an abrupt suspend.

Before sleep: lock and revoke screen/input grants first; stop new write admission; drain or cancel in-flight chunk writes without marking partial bytes verified; fsync checkpoint state and flush notes; terminate remote WebSockets, SSE and tracked HTTP/2 sessions, using graceful shutdown where possible before closing sockets. Preserve hub identity and certificate files. Never assume TCP, HTTP/2 streams or an in-memory authorization decision will survive sleep safely.

On wake: keep screen locked and backup mutations paused until data mounts, database recovery, certificate-valid clock and current revocation state are available. Re-enumerate interfaces, recreate invalid listeners and reannounce discovery when connectivity returns. Expire pairing/cleanup/session grants against their authoritative deadlines; invalidate any transient grant whose elapsed validity cannot be established. If wall-clock validity is uncertain, show a connection error and offer local time correction; never disable TLS time checks.

Reconnect with a fresh full mTLS handshake and current revocation/scope checks. For Linux v1 disable TLS resumption on this listener unless an independently tested policy rechecks current device authorization on every resumed connection; clear cached resumptions on sleep, revoke and policy changes. Existing clients should reconnect using their established retry behavior and query existing transfer status before resending missing chunks; probe completion before retrying an ambiguous finalize response. No new client protocol or blind mutation replay. Test actual companions: missing reconnect/resume behavior becomes a scoped compatibility fix and schedule gate, not an assumption that a server change solves it.

Add EDIT rows to the implementation scope: `hub/crates/hh-service/src/main.rs` wires the coordinator (~40–80 lines); `hub/crates/hh-net/src/lib.rs` tracks connection shutdown handles (~80–140); `hub/crates/hh-net/src/tls.rs` controls Linux resumption (~20–50); helper dispatch applies wake/lock revocation (~40–80). Add `hub/crates/hh-service/src/power.rs` (~200–300) and `tools/image-builder/tests/suspend-resume.sh` (~150–220) in P4.3. Test sleep mid-chunk/finalize, IP change, expired certificate, revocation while asleep, wrong clock, missing network and repeated suspend. Acceptance: no lost acknowledged data, no stale authenticated connection, automatic safe queue recovery, and fresh user consent for remote control/capture.

## 4. Shell and app architecture

### 4.1 Options A–D

| Option | Stack | Benefit | Cost / disposition |
|---|---|---|---|
| A | Tauri 2 + Svelte entire shell | Maximum immediate reuse | OS lock/window/input/capture policy still needs native components; resident WebView risks idle budget |
| B | All-native Rust UI, e.g. wgpu/iced | Consistent native surfaces | Rewrites functioning content views and increases scope |
| C | Native Rust compositor/chrome + Tauri app content | Keeps OS lifecycle native and reuses application behavior | Chosen; accessibility, multiwindow privileges and process isolation require explicit engineering |
| D | Electron-like resident shell | Familiar web packaging | Rejected by no-Electron rule and resource goals |

### 4.2 Processes and trust boundaries

```text
paired phone / Mac / iOS -- mTLS --> hh-service (non-root service identity)
                                           | core data, CA, existing /api
                               owner-checked local broker (Unix socket)
                                           |
     hh-compositor <------ authorized IPC ------> trusted Tauri content host
     [native chrome]                                  [Svelte apps]
           |                                              |
     hh-session / restricted system broker           private desktop.db
           |                                              |
     uinput/logind/PipeWire                       launch isolated web runner
           |                                    [Browser / YouTube processes]
     kernel + hardware                         no hub token, no private mounts
```

`hh-service` never runs as root. A narrowly scoped privileged launcher/broker supplies device handles or fixed operations when necessary; no arbitrary command execution. `hh-compositor` runs as the owner graphical session through logind. The service's local broker accepts authenticated peer UID plus active owner-session authorization, not a user-supplied owner ID alone. Separate typed operations from raw HTTP forwarding; native shell only receives the fields it needs. UI freezes do not stop backup jobs.

The local socket protocol is version 1, length-prefixed JSON, maximum frame 1 MiB, request ID + operation + payload, typed success/error, bounded event queue with snapshot refresh on overflow. Never accept filesystem paths for album or media IDs. Sessions expire on logout/lock; the service owns data operations. This is new IPC, not a replacement for companion APIs.

### 4.3 Native shell modules

For the baseline the modules below live in `hh-shell`, rendering GTK native widgets on labwc; labwc owns damage tracking, seats and window policy. The direct renderer/event loop and `hh-compositor` names describe the optional Smithay branch only. §5.4 maps every substitution, including lock/capture. Shared theme/data/state logic can be reused, but no custom-compositor work is required for the baseline.

Use one event-driven render loop with damage tracking. `theme.rs` consumes generated tokens; `status_bar.rs` draws clock/network/storage/device state; `launcher.rs` owns search and app grid; `window_manager.rs` owns focus/snap/workspaces and lifecycle; `lock_screen.rs` intercepts all seats and removes private capture; `oobe.rs` persists setup checkpoints; `notifications.rs` groups alerts; `quick_settings.rs` requests typed operations. Native controls publish an accessibility tree through AT-SPI. A renderer-only prototype does not satisfy usability.

Boot splash is a small early native/boot component; do not spawn WebKitGTK for startup. Timers tick clock at minute boundaries unless seconds are enabled; hardware changes use event subscriptions. Pause hidden animations and thumbnail work under transfer load.

### 4.4 Svelte reuse and permissions

Extract the nine existing views out of `App.svelte` incrementally into app components. Retain `api.js`, calculator logic, translation keys, `PhotoTile.svelte`, `Details.svelte`, private store and backend contracts. The main app becomes a route host for a compositor-issued app ID. Shared state modules own requests, selection, note save, music playback and private records; each view mounts only its own listeners/queries. Avoid creating nine copies of the whole monolith.

Current `trusted()` requires window label `main`; simply creating `photos` and `files` windows will break commands, and widening it to all labels would expose privileges. Replace it with a controller-owned registry mapping actual window identity + trusted bundled origin + owner session + app ID to allowed commands. Update the matching Tauri capabilities. Remote content gets no trusted entry or native commands. Block navigations of trusted app surfaces away from packaged content. On lock invalidate UI authorization before revealing any surface on unlock.

Albums currently have tables but no routes in the inspected dashboard/router. Add local album operations in a service-owned Unix broker using existing tables; do not invent public endpoints. Likewise, expose backup summary through local service delegation. Keep public clients unchanged. Notes/playlists/bookmarks remain owner-private and do not become shared hub records.

### 4.4.1 Album CRUD contract using existing tables

Source model is `0004_photos.sql`: `albums(id TEXT PRIMARY KEY, name TEXT NOT NULL, created_at INTEGER NOT NULL)` and `album_items(album_id REFERENCES albums ON DELETE CASCADE, file_id REFERENCES files ON DELETE CASCADE, added_at INTEGER NOT NULL, PRIMARY KEY(album_id,file_id))`. Add no table or owner column. These are hub-library albums, accessible to the authenticated local owner; future multi-user sharing needs a separate authorization design. Membership never grants permission to the underlying file.

| Local broker method | Request → response | Validation / transaction behavior |
|---|---|---|
| `albums.list` | `{cursor?,limit}` → `{items:[{id,name,created_at,item_count,cover_file_id?}],next_cursor}` | Limit 1–100; stable `(created_at,id)` ordering; counts/covers exclude trashed or unavailable media |
| `albums.create` | `{id,name}` → canonical album | Client-generated ULID makes retry idempotent; exact existing ID/name returns same record, conflicting name under same ID returns conflict |
| `albums.update` | `{id,expected_name,name}` → canonical album | Compare-and-set name; stale edit returns conflict; transaction serializes mutations |
| `albums.delete` | `{id}` → `{deleted:true}` | Confirm “Delete album? Photos stay in your library.” Idempotent; cascade membership only; never trash file rows/content |
| `albums.items.list` | `{id,cursor?,limit}` → ordered visible file records | Limit 1–100; stable `(added_at,file_id)` ordering; deterministic first visible item is cover |
| `albums.items.add` | `{id,file_ids}` → `{added,already_present}` | ≤100 unique file IDs; validate every ID is authorized, available media before one transaction; duplicate membership is a no-op |
| `albums.items.remove` | `{id,file_ids}` → `{removed}` | ≤100 IDs; idempotent removal of membership; never changes files/trash |

Normalize names to NFC, trim outer whitespace, reject empty/control-character names and names exceeding 120 Unicode scalar values or 480 UTF-8 bytes. Duplicate names across different album IDs are allowed; identity is the ULID, not the name. Use server UTC milliseconds for dates. No arbitrary SQL sorting or path input. Return typed invalid-input, not-found, conflict, permission-denied and storage-failure responses with no partial batch success. Authorize before revealing existence.

The existing schema has no manual order, custom cover or album trash; v1 does not imply those features. Soft-trashing a photo hides its membership from visible results but retains the row so restoring it restores membership; permanent file deletion cascades it. Deleting an album is irreversible metadata removal with explicit confirmation. CRUD belongs in `hh-db/src/queries.rs`, broker dispatch in `hh-net/src/local_ipc.rs`, UI in `Photos.svelte`. Add focused tests for duplicate requests, stale rename, foreign-key failure, hidden/restored media, empty album, paging ties and album-delete preserving file bytes. This is P4.4b work, not a schema migration.

### 4.5 Browser and YouTube isolation

Existing browser URL filtering and separate profile directories are useful controls, not a complete Linux sandbox. Introduce a separate web-runner process with a dedicated unprivileged identity, mount/PID/network namespaces, seccomp, a minimal writable profile/download root and no access to hub identity, private database, dashboard token, session socket or arbitrary system D-Bus. Run WebKit's own sandbox as well; never disable it to make a package build pass.

A restricted egress network namespace enforces public-only destination policy at connection time for IPv4/IPv6, including redirects, subresources, DNS rebinding, WebSockets and WebRTC. Permit only required DNS to a designated resolver, whose results still pass destination filtering; deny LAN/link-local/loopback/multicast and other non-global ranges. Deny raw sockets and CAP_NET_ADMIN in the runner. A trusted controller accepts only tab lifecycle, sanitized title/origin and bounded download metadata from it. Download content goes to quarantine; the owner confirms a separate trusted copy into Files. YouTube uses a separate profile and origin policy. Keep file dialogs and clipboard explicit and mediated.

Verify Tauri/WebKitGTK permission callbacks, profile separation, sandboxing, codecs, secure media handling and process behavior on the pinned Linux stack; Windows WebView2 success is not transferable evidence. [Tauri security model](https://v2.tauri.app/security/).

Browser isolation is **P4.4a**, a dedicated 4–5 solo-developer weeks before app migration P4.4b. Its scope includes runner/package integration, privilege/network isolation, quarantine, permissions and hostile-content tests. Stop that phase if a web process can reach the hub, protected sockets/data or privileged Wayland globals. Do not release Browser/YouTube, or quietly combine them into a trusted Tauri process, to recover schedule. A reduced release excluding both apps requires a product scope decision.

### 4.6 Wallpaper, icons and font asset pipeline

Use original locally stored SVG masters, not runtime generation or remote asset URLs. New sources: `desktop/assets/os/source/wallpaper.svg`, `desktop/assets/os/source/apps/{photos,files,browser,youtube,music,notes,calculator,devices,settings}.svg`, `desktop/assets/os/source/system-icons.svg`, `desktop/assets/os/manifest.json`, and `desktop/assets/os/LICENSES.md`. Manifest entries record stable asset ID, author/source, license, source hash, variant, export sizes and alt-label translation key. YouTube artwork is a generic video-app icon unless use of its brand artwork is separately cleared.

New `desktop/scripts/build-os-assets.mjs` validates/sanitizes SVG (no script, external links/fonts, embedded executable content), runs a pinned SVG renderer/optimizer and emits `desktop/assets/os/generated/`: wallpaper at 1280×800, 1920×1080 and 3840×2160 in a codec both renderers pass in the spike; app PNGs at 32/48/64/96/128/256 px; symbolic light/dark/high-contrast system SVG variants and a hashed manifest. Generate from one source palette; rasterize at build time, not every frame. Render center-crop previews for 16:9 and 16:10 with status/shelf safe areas. Keep only active-resolution textures decoded; measure decoded GPU/system memory separately from compressed bytes.

Package local Roboto/Noto Sans Devanagari files and licenses through the manifest, with Hindi shaping/fallback checks. Both native shell and Svelte resolve the same asset IDs to packaged resources; missing asset yields a generic icon, never a network fetch. New `desktop/scripts/check-os-assets.mjs` checks nine app IDs, dimensions, source/output hashes, forbidden SVG content, font licenses and contrast variants. Add `assets:build`/`assets:check` scripts to `desktop/package.json` and image-package install rules. Deliver reproducible exports, a contact sheet and English/Hindi visual review in P4.3; rerun checks only for asset changes. This replaces the earlier two-file placeholder asset row.

## 5. File-by-file change map

Each row specifies file(s), action, concrete work, dependencies, approximate added/changed lines, and phase. Where a row lists multiple filenames, each listed file is in scope; it is not permission for a directory-wide refactor. Preserve Windows feature gates and the existing test arrangement.

Routing rule for this map: common service/app files remain applicable; every `hh-compositor` row and custom DRM/capture/lock dependency is **Smithay-branch-only**, replaced by §5.4 in the labwc baseline. Browser runner/security rows marked P4.4 execute in P4.4a; other app rows execute in P4.4b. Do not implement both shell stacks. The asset pipeline in §4.6 and suspend map in §3.3 are additional exact scopes.

### 5.1 Existing files

| File | Action and implementation | Dependencies | Approx. lines | Phase |
|---|---|---|---:|---|
| `hub/Cargo.toml` | EDIT: register compositor; target-scoped Linux deps; explicit image build package selection excluding tray | Cargo, pinned Rust MSRV audit | 40–80 | P4.0/2 |
| `hub/Cargo.lock` | EDIT only when actual dependency work begins; pin selected versions, no speculative hand-edit | Cargo | generated | P4.0–6 |
| `hub/crates/hh-core/src/config.rs` | EDIT: production paths, immutable defaults + writable config override, migrate path config safely | existing serde/path types | 100–160 | P4.0 |
| `hub/crates/hh-core/src/platform.rs` | EDIT: document full existing trait semantics and capability behavior; no incompatible signatures | existing DTOs | 30–60 | P4.0 |
| `hub/crates/hh-core/src/paths.rs` | EDIT only to add Linux symlink/path-jail regression coverage and safe directory operations | Unix file APIs | 70–140 | P4.0 |
| `hub/crates/hh-auth/src/ca.rs` | EDIT: atomic restricted Unix files, complete-identity checks, validated import/export entry points | existing crypto, new wrapping codec | 180–280 | P4.0/5 |
| `hub/crates/hh-auth/src/lib.rs` | EDIT: expose migration module without raw-key logging interfaces | ca/migration modules | 10–20 | P4.5 |
| `hub/crates/hh-auth/Cargo.toml` | EDIT: encrypted-bundle dependencies, platform gates | reviewed AEAD/KDF libraries | 10–20 | P4.5 |
| `hub/crates/hh-db/src/queries.rs` | EDIT: bounded transactional album operations and backup snapshot helper as needed | existing schema/rusqlite | 120–200 | P4.4/5 |
| `hub/crates/hh-db/Cargo.toml` | EDIT: enable SQLite backup support if not already enabled | rusqlite | 1–5 | P4.5 |
| `hub/crates/hh-storage/src/library.rs` | EDIT: volume mapping, platform-neutral path handling; preserve same-volume finalize | core config, mount IDs | 60–120 | P4.0 |
| `hub/crates/hh-storage/src/health.rs` | EDIT: Linux disk provider wiring; unsupported health stays unknown | hh-hw adapter | 40–80 | P4.0 |
| `hub/crates/hh-storage/src/lib.rs` | EDIT: migration verifier module export only | migration module | 5–10 | P4.5 |
| `hub/crates/hh-remote/src/lib.rs` | EDIT: Linux module wiring behind cfg; retain session authorization | Linux adapters | 20–40 | P4.0 |
| `hub/crates/hh-remote/src/helper_client.rs` | EDIT: production runtime socket selection, peer/session checks, bounded request timeouts | Unix socket support | 100–160 | P4.0 |
| `hub/crates/hh-remote/Cargo.toml` | EDIT: Linux-only input/D-Bus bindings | evdev or narrow ioctl wrapper, zbus | 10–25 | P4.0 |
| `hub/crates/hh-hw/src/audit.rs` | EDIT: merge real Linux fields with existing report/rating logic | new Linux module | 50–100 | P4.0 |
| `hub/crates/hh-hw/src/lib.rs` | EDIT: export/select Linux health provider | sysfs/udev | 20–40 | P4.0 |
| `hub/crates/hh-hw/src/network.rs` | EDIT: dispatch Linux network state/hotspot through adapter | NetworkManager D-Bus | 60–120 | P4.0/3 |
| `hub/crates/hh-hw/Cargo.toml` | EDIT: target-specific hardware/network bindings | zbus, udev if needed | 10–25 | P4.0 |
| `hub/crates/hh-stream/src/lib.rs` | EDIT: Linux capture lifecycle and truthful capability wiring | helper capture backend | 80–160 | P4.3 |
| `hub/crates/hh-stream/Cargo.toml` | EDIT: Linux capture feature dependency wiring | PipeWire/encoder bindings | 10–25 | P4.3 |
| `hub/crates/hh-session/src/main.rs` | EDIT: choose Linux dispatch under cfg, retain Windows/default dev paths | existing protocol | 40–80 | P4.0 |
| `hub/crates/hh-session/src/server.rs` | EDIT: secure production socket, SO_PEERCRED, size/rate/time limits | Unix APIs | 120–180 | P4.0 |
| `hub/crates/hh-session/src/dispatch.rs` | EDIT: route existing methods to Linux providers and check lock/consent | Linux module | 60–100 | P4.0/3 |
| `hub/crates/hh-session/Cargo.toml` | EDIT: target-gated Linux capture/input/system bindings | selected bindings | 20–40 | P4.0/3 |
| `hub/crates/hh-service/src/main.rs` | EDIT: production Linux host, readiness, provider injection, migration guard, local broker spawn | systemd host, hh-net | 120–200 | P4.0/3/5 |
| `hub/crates/hh-service/src/local_security.rs` | EDIT: distinct service/UI permissions; bounded local-broker credential handoff | Unix permissions/peer auth | 80–140 | P4.0 |
| `hub/crates/hh-service/Cargo.toml` | EDIT: Linux-only systemd notify integration | sd-notify or bounded native protocol | 10–20 | P4.0 |
| `hub/crates/hh-net/src/lib.rs` | EDIT: expose internal broker and bridge state/events | local IPC module | 20–40 | P4.3/4 |
| `hub/crates/hh-net/src/admin.rs` | EDIT: select Linux providers behind existing handlers; retain response shapes | provider interfaces | 60–100 | P4.0/3 |
| `hub/crates/hh-net/src/routes.rs` | EDIT only provider/capability integration, no renamed/new public paths | same | 40–80 | P4.0/3 |
| `hub/crates/hh-tools/src/main.rs` | EDIT: register migration and performance commands | new tool modules | 50–90 | P4.5/6 |
| `hub/crates/hh-tools/Cargo.toml` | EDIT: link migration/measurement facilities | existing workspace crates | 10–20 | P4.5 |
| `desktop/src/App.svelte` | EDIT: extract nine views; retain Windows route host; OS app-ID mode | new app components/state | 250–450 changed plus moved code | P4.4 |
| `desktop/src/api.js` | EDIT: OS local commands and generic platform wording; retain existing HTTP/media contracts | Tauri bridge | 60–100 | P4.4 |
| `desktop/src/style.css` | EDIT: consume generated tokens, app-only chrome, touch/contrast/scale modes | os-tokens | 120–200 | P4.4 |
| `desktop/src/i18n.js` | EDIT: externalize OS app strings and Linux platform messages in English/Hindi | existing translator | 180–300 | P4.3/4 |
| `desktop/src-tauri/src/main.rs` | EDIT: per-app trusted registry, lifecycle flush, Linux broker path, runner control; Windows owner-approved migration export launch/progress without returning keys to JavaScript | app_registry, local IPC, hh-tools migration | 240–380 | P4.4/5 |
| `desktop/src-tauri/src/proxy.rs` | EDIT: Linux owner-broker transport selection; Windows fixed-loopback remains | local broker | 80–140 | P4.4 |
| `desktop/src-tauri/src/store.rs` | EDIT: owner path config; atomic app preferences/history; album UI state must not impersonate shared album data | existing records | 70–120 | P4.4/5 |
| `desktop/src-tauri/src/browser.rs` | EDIT: Linux runner launch/control; keep Windows behavior; quarantine download flow | web runner | 160–240 | P4.4 |
| `desktop/src-tauri/capabilities/trusted-shell.json` | EDIT: explicit trusted app identities; remote sources have zero privileges | matching Rust registry | 30–60 | P4.4 |
| `desktop/src-tauri/Cargo.toml` | EDIT: Linux-only IPC/runner deps; preserve Windows dependencies | Tauri, Unix bindings | 20–40 | P4.4 |
| `desktop/src-tauri/tauri.conf.json` | EDIT: Linux app window/bootstrap and CSP; no privilege wildcard | Tauri capabilities | 25–50 | P4.4 |
| `desktop/package.json` | EDIT: token generation, asset build/check and app build scripts | existing Vite/Svelte, pinned asset tools | 10–25 | P4.3/4 |

No planned runtime edits in `hh-transfer`, `hh-photos` core engines, `hh-tray`, Apple/Android client code or any existing SQL migration. Add narrowly scoped compatibility tests where needed. If inspection during implementation uncovers necessary wire/schema changes, stop that slice for a contract decision instead of making them incidental.

### 5.2 New Rust and frontend files

| New file(s) | Concrete contents | Dependencies | Approx. lines | Phase |
|---|---|---|---:|---|
| `hub/crates/hh-remote/src/linux.rs` | Full InputControl adapter, key release tracking, logind PowerControl, secure-surface denial | uinput, zbus, xkbcommon | 350–550 | P4.0 |
| `hub/crates/hh-hw/src/linux.rs` | sysfs/udev disk/audit collection, bounded smartctl JSON parser | serde_json, smartmontools | 300–450 | P4.0 |
| `hub/crates/hh-hw/src/network_linux.rs` | Wi-Fi/Ethernet/rfkill/hotspot state and controlled changes | NetworkManager/zbus | 180–280 | P4.0/3 |
| `hub/crates/hh-service/src/systemd.rs` | ServiceHost adapter, SIGTERM, readiness/watchdog, shutdown flushing | tokio, notify interface | 120–200 | P4.0 |
| `hub/crates/hh-session/src/platform_linux.rs` | Existing helper method dispatch, active seat/owner checks | remote/hw Linux adapters | 180–300 | P4.0 |
| `hub/crates/hh-session/src/capture_linux.rs` | PipeWire consumer, encoder selection, lifecycle and grant checks | PipeWire, validated encoders | 350–600 | P4.3 |
| `hub/crates/hh-stream/src/linux.rs` | Capture service coordination, limits and capability report | session helper | 140–220 | P4.3 |
| `hub/crates/hh-core/src/local_ipc.rs` and edit `hub/crates/hh-core/src/lib.rs` | Versioned DTOs/errors, size limits, owner/session handles; module export | serde | 220–320 + 5 | P4.0/3 |
| `hub/crates/hh-net/src/local_ipc.rs` | Service-owned Unix listener for bounded shell snapshots, albums/backup reads and mutations | existing services/DB | 300–500 | P4.3/4 |
| `hub/crates/hh-auth/src/migration.rs` | Authenticated passphrase-wrapped identity bundle, key matching/fingerprint verification | AEAD/KDF, existing cert library | 250–400 | P4.5 |
| `hub/crates/hh-storage/src/migration.rs` | Manifest paths/copy verification/root remap and resumable checkpointing | BLAKE3, existing journals | 350–550 | P4.5 |
| `hub/crates/hh-tools/src/migration.rs` | Windows export, offline verify/import CLI and structured results | auth/storage/SQLite snapshot | 300–450 | P4.5 |
| `hub/crates/hh-tools/src/os_metrics.rs` | Boot marker, process-tree RSS/PSS, whole-system memory and CPU sampling | procfs/cgroups | 180–260 | P4.6 |
| `hub/crates/hh-compositor/Cargo.toml` | Native shell/compositor crate, platform features and build token inputs | Smithay, rendering/text, AT-SPI, zbus | 60–100 | P4.2 |
| `hub/crates/hh-compositor/src/main.rs`, `src/lib.rs` | Lifecycle, nested/DRM backend, render/event loop, connection wiring | Smithay/logind | 120–180 / 600–1000 | P4.2 |
| `hub/crates/hh-compositor/src/status_bar.rs` | Clock/network/battery/storage/device indicators and accessible actions | snapshot/theme | 180–260 | P4.3 |
| `hub/crates/hh-compositor/src/launcher.rs` | Nine-app grid, local search, shelf/pins/recents | app registry | 250–350 | P4.2 |
| `hub/crates/hh-compositor/src/window_manager.rs` | xdg-shell lifecycle/focus, floating/split/fullscreen, two workspaces | Smithay | 650–1000 | P4.2 |
| `hub/crates/hh-compositor/src/notifications.rs` | Banners, center, DND, redaction, bounded persisted history | local IPC/JSON | 250–350 | P4.3 |
| `hub/crates/hh-compositor/src/lock_screen.rs` | Owner auth, secure seat handling, capture/input revocation, recovery entry | auth broker | 300–450 | P4.3 |
| `hub/crates/hh-compositor/src/oobe.rs` | Language/network/migration/owner/pairing wizard, resumable checkpoints | local IPC | 350–500 | P4.3/5 |
| `hub/crates/hh-compositor/src/quick_settings.rs` | Network/audio/brightness/sharing/power controls with pending/error states | D-Bus broker | 250–350 | P4.3 |
| `hub/crates/hh-compositor/src/theme.rs`, `src/accessibility.rs`, `src/input.rs` | Generated visual tokens; semantic AT-SPI tree; shortcuts/IME/touch/virtual keyboard routing | token generator, AT-SPI, text-input | 120–180 / 350–550 / 350–550 | P4.2/3 |
| `hub/crates/hh-compositor/src/capture.rs`, `src/ipc.rs`, `src/apps.rs` | Authorized frame producer; owner IPC; signed/bundled app manifest registry | PipeWire, local DTOs | 300–500 / 180–260 / 180–260 | P4.2/3 |
| `desktop/src/apps/{Photos,Files,Browser,YouTube,Music,Notes,Calculator,Devices,Settings}.svelte` | Nine exact extracted views; app-specific lifecycle/status/accessibility | existing App.svelte logic | mostly moved code, 80–220 integration per view | P4.4 |
| `desktop/src/state/{hub,private,music,notes}.js` | Request lifecycle, existing records, queue, durable note flush | existing api/store | 120–220 each | P4.4 |
| `desktop/src/theme/os-tokens.json`, `desktop/scripts/generate-os-tokens.mjs` | Single token source; emit Rust constants and CSS deterministically | Node, serde build input | 100–160 / 100–160 | P4.2/4 |
| `desktop/src-tauri/src/app_registry.rs`, `src/os_bridge.rs` | Authenticated window/command registry; owner-scoped local IPC | Tauri, local DTOs | 200–300 / 180–260 | P4.4 |
| `desktop/web-runner/Cargo.toml`, `src/main.rs`, `src/control.rs`, `src/downloads.rs` | Standalone untrusted content host; bounded controller protocol; quarantine manager | WebKitGTK, reviewed sandbox launcher | 50–80 / 250–400 / 160–240 / 160–240 | P4.4 |
| `desktop/assets/os/source/`, `desktop/assets/os/manifest.json`, `desktop/assets/os/LICENSES.md`, `desktop/scripts/{build-os-assets,check-os-assets}.mjs` | Exact masters/exports/checks enumerated in §4.6; replaces the original two-asset sketch | pinned renderer, original/licensed assets | asset work + 120–200 lines per script | P4.3 |

Brace lists above enumerate proposed files, not shell commands. Generate only the required app components; keep the existing shared PhotoTile/Details components in place unless extraction requires import changes.

### 5.3 Image builder, system policies and installer files

All paths below are new under `tools/image-builder/` unless stated otherwise. The `rootfs-overlay` prefix means files copied to the corresponding absolute root in the generated image, not written to the development machine's `/etc`.

| New path | Work and dependencies | Approx. lines | Phase |
|---|---|---:|---|
| `README.md`, `sources.lock.json` | Reproducible Linux build host setup, pins/hashes/licenses, reference hardware manifest | 180 / 100 | P4.1 |
| `buildroot-config/homehub_x86_64_defconfig`, `buildroot-config/linux.fragment` | glibc/systemd desktop dependencies and validated kernel/firmware requirements | 180 / 140 | P4.1 |
| `external.desc`, `Config.in`, `external.mk` | Buildroot external tree registration | 10 / 30 / 20 | P4.1 |
| `package/homehub/Config.in`, `package/homehub/homehub.mk` | Rust/Svelte/runner package builds and install rules | 30 / 130 | P4.1/4 |
| `board/homehub/genimage.cfg`, `board/homehub/grub.cfg` | Disk geometry, slots, boot-attempt fallback; keep-Windows entry handling | 130 / 100 | P4.1/5 |
| `scripts/build-image.sh` | Reproducible build and image verification with SBOM/license outputs | 120–180 | P4.1 |
| `scripts/create-usb.sh` | Host disk enumeration, reject mounted/system disks, display exact target and require erase confirmation, write/readback verify | 180–260 | P4.5 |
| `scripts/create-usb.ps1` | Same guarded Windows USB writing workflow | 180–260 | P4.5 |
| `scripts/update-bundle.sh` | RAUC bundle manifest, offline signing interface, compatible-device and version metadata | 120–180 | P4.6 |
| `rootfs-overlay/etc/systemd/system/homehub.service` | Non-root service; bounded writable paths/capabilities, readiness/watchdog, restart policy | 35–60 | P4.0/1 |
| `rootfs-overlay/etc/systemd/system/homehub-session.service` | Active session helper lifecycle and restricted device access | 35–60 | P4.0/2 |
| `rootfs-overlay/etc/systemd/system/homehub-compositor.service` | Owner seat, compositor startup, locked restart and crash limit | 35–60 | P4.2 |
| `rootfs-overlay/etc/systemd/system/homehub-health.service` | Boot slot health and explicit mark-good; not just a timer | 25–40 | P4.6 |
| `rootfs-overlay/usr/libexec/homehub-health` | Service/DB/data/shell-ready checks and 60-second stability gate | 100–150 | P4.6 |
| `rootfs-overlay/usr/libexec/homehub-system-broker` | Fixed operations for power/devices/network/sandbox launch; owner/session/lock policy, no arbitrary exec | Compiled from the Rust source below, not a second implementation | P4.0/3/4 |
| `system-broker/Cargo.toml`, `system-broker/src/main.rs`, `system-broker/src/policy.rs` | Actual source for previous installed binary, least-privilege methods and peer checks | 50 / 300–450 / 150–250 | P4.0/3 |
| `rootfs-overlay/etc/systemd/system/homehub-system-broker.service` | Restricted privileged broker with explicit capabilities/device allowlist | 40–70 | P4.0/3 |
| `rootfs-overlay/etc/udev/rules.d/70-homehub.rules` | Narrow runtime device grants; never chmod all input nodes world-writable | 25–40 | P4.0 |
| `rootfs-overlay/etc/polkit-1/rules.d/50-homehub.rules` | Fixed logind/NetworkManager operations for active owner/broker | 60–90 | P4.3 |
| `rootfs-overlay/etc/nftables.d/homehub.nft`, `rootfs-overlay/etc/homehub/web-sandbox.json` | Service listener policy, isolated web egress, namespace/mount/seccomp specification | 100–160 / 100–160 | P4.4 |
| `rootfs-overlay/etc/rauc/system.conf` | Slot identities, bootloader integration, update trust root location | 40–70 | P4.6 |
| `rootfs-overlay/etc/homehub/defaults.json`, `rootfs-overlay/usr/share/homehub/apps.json` | Immutable service defaults and nine-app launch manifest | 60 / 100 | P4.1/2 |
| `installer/Cargo.toml`, `installer/src/main.rs`, `installer/src/storage.rs`, `installer/src/flow.rs`, `installer/src/recovery.rs` | Native live installer: audit → dry run → target confirmation → image write → verified restore → boot; recovery/rollback | 60 / 160 / 400 / 450 / 250 | P4.5 |
| `tests/boot-smoke.sh`, `tests/ab-rollback.sh`, `tests/web-isolation.sh`, `tests/migration-roundtrip.sh` | Focused VM probes/fault injection for corresponding features | 120–200 each | P4.1/4/5/6 |
| `tests/protocol-compat.sh`, `tests/idle-budget.sh` | Paired-client unchanged contracts; repeatable memory/CPU/boot report | 100–160 each | P4.0/6 |
| `installer/src/trial.rs`, `rootfs-overlay/etc/systemd/system/homehub-trial.service`, `rootfs-overlay/etc/homehub/trial.json`, `trial/sample-manifest.json`, `tests/live-trial.sh` | RAM-only live trial, loopback sample service, no automatic internal-disk mounts/writes or LAN advertisement, explicit install/export handoff and before/after disk-hash tests | 200 / 40 / 60 / 60 / 180 | P4.1/5 |

New documentation deliverables are exactly `docs/PHASE4_UI_UX_DESIGN.md`, `docs/PHASE4_IMPLEMENTATION_PLAN.md`, and `docs/PHASE4_MIGRATION_GUIDE.md`. No installer, image builder or runtime changes are implemented as part of this planning request.

### 5.4 Concrete labwc fallback / baseline file map

labwc runs as the owner compositor; `hh-shell` is a separate Rust/GTK4 + gtk4-layer-shell native client. Layer-shell supplies panel/launcher/notification surfaces; foreign-toplevel management supplies app listing/activation/close. Neither protocol alone supplies arbitrary snapping/workspace control or a secure lock. Use validated labwc bindings/configuration for window operations, a separate session-lock client for lock, and a narrow adapter for any missing command; if an operation is unsupported at the pinned version, record a blocker rather than invent an API. The [labwc integration guide](https://labwc.github.io/integration.html) documents the split between compositor and desktop clients; [GTK4 Layer Shell](https://github.com/wmww/gtk4-layer-shell/blob/main/README.md) provides native panel integration with Rust bindings. All required protocols/builds must pass the Week 1 matrix on selected versions.

All following paths are NEW unless EDIT is stated. Relative `src/` paths in a grouped row belong to the fully named crate in that row. Estimates are implementation sizing, not proof that the component exists.

| File(s) | Work replacing Smithay branch | Dependencies / estimate | Phase |
|---|---|---|---|
| `hub/Cargo.toml` (EDIT) | Register `hh-shell` and `hh-lock`, omit `hh-compositor` in baseline image package list | Cargo, ~20 lines | P4.2 |
| `hub/crates/hh-shell/Cargo.toml`, `src/main.rs`, `src/lib.rs` | Native session client lifecycle, GTK event loop, app/data broker wiring | gtk4, gtk4-layer-shell, Wayland client, ~60/120/200 lines | P4.2 |
| `hub/crates/hh-shell/src/{status_bar,launcher,notifications,quick_settings,oobe,theme,ipc,apps}.rs` | Same product state/content as corresponding custom-shell modules; GTK widgets, layer surfaces, shared tokens | ~120–400 lines per module; no DRM/seat ownership | P4.2/3 |
| `hub/crates/hh-shell/src/{window_adapter,input_mode,accessibility}.rs` | Toplevel registry and supported actions; boot mode preference; GTK semantics/focus/IME/OSK integration | ~200–350 each; compositor owns physical input | P4.2/3 |
| `hub/crates/hh-shell/src/trial.rs` | Trial badge, compatibility report, volatile-data warning and validated install handoff; register in `lib.rs` | trial service config, ~180–260 | P4.1/5 |
| `hub/crates/hh-lock/Cargo.toml`, `src/main.rs`, `src/auth.rs` | Dedicated session-lock client, one secure surface per output, authenticated owner unlock, locked crash/restart behavior | ext-session-lock + approved native UI/auth integration, ~60/300/180; screen-reader and protocol viability are gates | P4.3 |
| `hub/crates/hh-session/src/capture_labwc.rs` | Authorized capture through validated labwc-compatible portal/PipeWire backend; tear down grant/node before lock | portal backend, PipeWire, ~250–400 | P4.3 |
| `tools/image-builder/system-broker/src/wayland_policy.rs` | Launch untrusted clients via enforced restricted Wayland connection, hiding capture/virtual-input/data-control/session-lock globals; do not trust client-supplied app ID | protocol-aware filtering/validated isolation implementation, ~300–600 plus negative tests | P4.4a |
| `tools/image-builder/rootfs-overlay/etc/xdg/labwc/{rc.xml,autostart,environment}` | Trusted shell startup, key bindings, snap/workspaces, touch defaults, decorations and restart policy | pinned labwc supported config, ~160/30/20 | P4.2 |
| `tools/image-builder/rootfs-overlay/etc/systemd/system/{homehub-labwc,homehub-shell,homehub-lock}.service` | Start compositor/shell/locker in owner session; reconnect shell on crash; never relaunch unlocked after compositor failure | systemd/session lifecycle, ~40 each | P4.2/3 |
| `tools/image-builder/rootfs-overlay/etc/xdg/xdg-desktop-portal/homehub-portals.conf` | Select validated screenshot/screencast backend and enforce broker consent | portal backend verified in spike, ~25 | P4.3 |
| `tools/image-builder/package/homehub/homehub.mk`, `buildroot-config/homehub_x86_64_defconfig` (planned files, EDIT when created) | Package labwc, GTK4 native shell/lock, GTK/WebKit app dependencies, portal and OSK; omit custom compositor | Pinned packages and measured RSS | P4.1/2 |
| `tools/image-builder/tests/{labwc-shell,lock-isolation,wayland-policy}.sh` | Panels/focus/app lifecycle; locker death/display changes; hostile clients cannot capture/inject/read selections | Disposable VM tests, ~150–250 each | P4.2/3/4a |

Common app extraction, local broker, assets, migrations, updater and device APIs remain unchanged. Use `hh-shell` in health checks instead of `hh-compositor`, and require labwc plus locker readiness. Disable Xwayland for v1 unless a separately tested dependency requires it; it must not become a shortcut around isolation. Before showing an owner credential or destructive prompt, revoke remote grants, release held uinput state and inhibit the helper until the secure flow ends. This replaces custom compositor event tagging; no assumption that labwc labels synthetic input securely.

GTK4 adds real resident cost alongside WebKitGTK's own dependency set; measure both in the spike. Failure of lock semantics, accessible unlock, restricted capture or the memory requirement stops release. The fallback avoids authoring a compositor; it does not make shell/security/accessibility work free.

## 6. Design-to-code traceability

In the labwc baseline, native rows below resolve to `hh-shell`, `hh-lock`, `window_adapter` and `capture_labwc` via §5.4; the original module names otherwise denote the optional Smithay branch.

| Design element / screen | Owning files | Data / acceptance condition |
|---|---|---|
| S01 boot/OOBE/lock | image boot config; compositor oobe/lock/input; system broker | Offline setup, encrypted-data prompt, fail-closed lock, redacted status |
| S02 Home, shelf, workspaces | launcher/apps/window_manager/theme | Nine launch/focus/close paths; no resident WebView at idle |
| S03 Photos | Photos.svelte, PhotoTile.svelte, local broker, DB queries | Timeline, existing-table albums, viewer, backup summary and safe duplicate review |
| S04 Files | Files.svelte, Details.svelte, existing storage/API | Category/search/detail/trash; no source destruction during copy |
| S05 Browser / S06 YouTube | respective views, browser.rs, web runner, sandbox policies | Tabs/bookmarks/quarantined downloads and independent YouTube profile; no local privilege access |
| S07 Music | Music.svelte, music.js, store, audio broker | Library/playlists/queue and media keys; playback survives minimize |
| S08 Notes | Notes.svelte, notes.js, store | Search/edit/trash; forced save failure never loses draft |
| S09 Calculator | Calculator.svelte, existing calculator.js, preferences | Safe expression parsing and bounded history |
| S10 Devices | Devices.svelte, unchanged pairing/device APIs | Pair consent, permission editing, immediate revoke |
| S11 Settings | Settings.svelte, os_bridge, system broker, updater | Eight categories and truthful unsupported controls |
| S12 Notifications | notifications/status_bar/local broker | DND/grouping/focus return; dismiss is not health recovery |
| S13 Quick settings | quick_settings/network_linux/system broker | Wi-Fi/audio/brightness/storage/devices/power with pending and failure states |
| Tokens / English-Hindi / accessibility | os-tokens, theme, i18n, accessibility/input | Consistent contrast/scale and complete screen-reader path |
| Phone remote / capture | remote Linux, session Linux/capture, compositor input/capture | Existing protocols; revoke/lock stop input and frames |

## 7. Implementation sequence, staffing and stop gates

The original 16-week outline is withdrawn as a delivery estimate. **Solo developer: 32–40 working weeks** for the labwc baseline, including two weeks of explicit contingency; **small team of 2–3 experienced engineers: provisionally 20–26 calendar weeks**, after the spike, with platform, shell and security/app work overlapped only where dependencies permit. These are planning estimates, not measured throughput or release promises. They assume the developer already knows Rust/Linux, usable reference hardware, timely security/design review and no major failed gate. Pilot recruitment/waiting, procurement and unsupported hardware can extend calendar time.

A custom Smithay compositor is a separate **12–24-week workstream**, not included in the labwc estimate. For a solo developer, reserve a conservative 44–64-week combined planning envelope before deciding what proven work can replace baseline tasks. Do not count the same person's hours as parallel capacity. Passing Week 1 proves integration feasibility only; it does not shorten production compositor work to three weeks.

| Phase | Solo weeks | Ordered scope | Stop-and-reassess gate |
|---|---:|---|---|
| P4.S | 1 | Buildroot VM + actual Tauri/WebKitGTK app; Smithay harness and labwc native-panel matrix | G0, end of Week 1: select proven baseline; missing chain evidence blocks commitment; no funded custom track means labwc |
| P4.0 | 3–4 | Linux configuration, secure helpers, traits, systemd, unchanged API/client smoke | G1: service/identity/transfer/revocation work on Linux; unresolved trust or data-integrity failures stop image integration |
| P4.1 | 3–4 | Reproducible base image, hardware enablement, slots, recovery boot and live trial foundation | G2: boot reference laptop and packaged app; image dependencies and preliminary resource envelope measured; failed budget triggers explicit decision |
| P4.2 | 4–5 | labwc native Home/shelf, window adapter, touch detection, IME/accessibility and assets foundation | G3: app lifecycle/focus/snap, Hindi, native screen-reader smoke and input modes pass; missing protocol means revise implementation before chrome expansion |
| P4.3 | 4–5 | Status/notifications/settings, secure lock, OOBE, audio/network/power, capture and mTLS resume, final assets | G4: lock/crash/capture isolation, accessible unlock and suspend/reconnect pass; no exceptions for secure flows |
| P4.4a | 4–5 | Dedicated web runner/sandbox, network and Wayland restrictions, quarantine and permissions | G5: hostile-web matrix passes; Browser/YouTube integration cannot ship ahead of isolation |
| P4.4b | 4–5 | Extract nine apps, trusted registry, complete album CRUD, notes/music state and English/Hindi | G6: all nine function; album-delete preserves files; save failures retain notes; app restart/low-memory behavior passes |
| P4.5 | 4–5 | USB/live installer, Windows export, migration/OOBE resume, trial UX and recovery | G7: L1–L3 installs plus DPAPI identity continuity, Keep/Replace/Trial and power-cut/hash/collision checks; no user-data destructive pilot before pass |
| P4.6 | 3–4 | Signed A/B, rollback, measured resource/boot budgets, hardware/accessibility/polish | G8: tamper/bad-slot rollback and actual performance/accessibility gates pass; then existing release/pilot gates |
| Contingency | 2 | Integration fixes uncovered by the gates | Not permission to waive gates; exhaustion triggers a new estimate |
| **Total** | **32–40** | **labwc baseline** | **Failed gates add time or require an explicit scope/architecture decision** |

At every G0–G8 record owner, image/commit, pass/fail evidence, blockers and the revised remaining estimate in the active slice/decision ledger. Stop dependent work on failure; continue only independent authorized work. No silent scope cuts, implied passes, repeatedly extended spikes or borrowing the migration/security allowance to preserve a date. Work one bounded `docs/CURRENT.md` slice at a time and run targeted checks only. Three reference laptops are development gates; the existing ≥10 supported-model release criterion and household pilot requirements remain prerequisites unless explicitly changed.

## 8. Targeted testing and performance evidence

### 8.1 Test layers

Run only the checks relevant to the active slice; this documentation task does not run or require a full product suite. Example implementation commands (on the stated platform, after files exist):

```sh
# Linux port slice, in a Linux VM with image toolchain dependencies
cargo build --manifest-path hub/Cargo.toml --target x86_64-unknown-linux-gnu -p hh-service -p hh-session -p hh-tools
cargo test --manifest-path hub/Cargo.toml -p hh-remote
# App extraction slice
npm --prefix desktop run check
npm --prefix desktop test
# Image slices, only after scripts have been implemented
bash tools/image-builder/tests/boot-smoke.sh
bash tools/image-builder/tests/migration-roundtrip.sh
bash tools/image-builder/tests/ab-rollback.sh
```

Use `cargo test -p <changed-crate>` with relevant test filters where practical, formatting for edited Rust files, and linting on changed packages. Windows hardware/installer/WebView2 checks remain pending on this Mac. Merely adding a Rust target on macOS is insufficient to validate Linux graphics/audio/system integration. Do not retry the same failing command more than twice; record missing dependencies or platform blockers.

| Area | Focused tests |
|---|---|
| Trait adapters | Fake sysfs/SMART fixtures, unsupported devices, logind denied, rate limits, missing uinput, held-key disconnect |
| Security | Wrong Unix UID/session, oversized/truncated IPC, symlinked keys/sockets, remote secure-prompt input, revoked device, lock during capture |
| Apps | Real trusted-window registry, no command access from external origin, note-save failure/close race, playlist missing files, albums commit failure, calculator malformed input |
| Browser | Top-level/subresource/redirect/DNS-rebinding IPv4/IPv6 LAN access, WebRTC, token/private-file/socket reads, download traversal and execution denial |
| Storage/migration | SQLite snapshot + integrity/foreign-key checks, BLAKE3 per chunk/root, NTFS collision/reparse/ADS/EFS cases, disk-full, interrupted rename/import, active cleanup leases |
| Updates | Truncated/tampered/wrong-product bundle, power cut at each slot write/activation step, three failed boots, health failure, old slot reading current Data |
| Accessibility | Orca/native AT-SPI and WebView semantics, keyboard-only OOBE/lock, Hindi IME, 200% text, focus return, high contrast, reduced motion |

QEMU with disposable disk images is primary for boot/install/update faults; nested compositor testing covers development. VirtualBox may provide a secondary boot check, not proof of DRM acceleration, Wi-Fi, uinput, suspend or real encode behavior. Never run fault injection on the user's actual library.

### 8.2 Reference laptops and metrics

L1: older Intel integrated graphics + SATA SSD + Intel Wi-Fi; L2: AMD integrated graphics + NVMe + Realtek Wi-Fi; L3: older BIOS-capable mixed GPU/Wi-Fi machine with HDD and USB fallback networking. These are selection profiles, not verified product recommendations. Record exact model, firmware, PCI IDs, kernel/firmware image, storage and results before calling a model supported. Unsupported L3 hardware gets an honest limitation, not a universal-support claim.

Measure cold power-button-to-interactive time over ten runs, report median/p95 and firmware time separately; target p95 <15 s on supported reference hardware. Passphrase entry time is reported separately, not hidden. Measure unlocked native Home after five minutes settled, no user apps open, no transfers, network connected, regular health services enabled. Sample CPU over ten minutes; aggregate relative to one logical CPU and report host core count. Target <3% average with no repeated spikes from polling; service target <1%.

Memory is measured two ways: sum resident RSS per process tree/cgroup (report shared-page double counting), and system physical usage using `/proc/meminfo` with reclaimable caches identified. Report PSS, compositor/GPU allocations where observable, kernel/slab, and `MemTotal-MemAvailable`; do not label service RSS “entire OS memory.” **Both total process RSS and measured non-reclaimable system usage must fit <200 MB under the defined idle condition**, or the stated whole-system budget has not passed. Disclose kernel/GPU reservations that cannot be accounted for. No swap usage is allowed to disguise a failure.

Allocation planning target: service ≤65 MB, compositor/chrome ≤45 MB, system/network/audio helpers ≤40 MB, kernel/other allowance ≤35 MB, 15 MB contingency. These are unverified allocations. Prefer a static wallpaper, lazy WebViews, bounded caches, demand-start capture and inactive-tab unload. Do not strip certificate checks, sandboxing or accessibility to hit the target. Measure Photos, video playback and 12 browser tabs separately, enforce process/cgroup limits, and test graceful low-memory failure with ongoing backups.

Install target: supported reference SSD, prepared USB, OS installation and first interactive setup <15 minutes in a non-technical usability test. Report image write/verify/setup separately; bulk user-data copying is capacity-dependent and receives its own estimate. Keep Windows resize preparation, BitLocker recovery and large migration cannot honestly be included in a universal 15-minute promise.

## 9. First implementation slice and readiness

When implementation is requested, replace the completed CURRENT slice with **P4.S — Week 1 VM dependency-chain feasibility**, limited to the spike files in §2.3.1. Exit only with the guest-image/app evidence and G0 decision; a blocked toolchain/VM is recorded as a blocker. Then begin P4.0a Linux configuration and secure startup. This planning revision does not execute the spike, install dependencies, or reformat any disk.

Unresolved acceptance evidence, with owners: platform engineer — actual Buildroot/WebKitGTK compatibility and hardware matrix; shell engineer — secure compositor, IME and AT-SPI gates; frontend/security engineer — separate browser sandbox; migration engineer — original-Windows DPAPI export and companion pin behavior; QA — resource/boot/installer/update measurements and pilot gates. These are concrete tests to perform, not unspecified architecture choices.

## 10. Repository references

Review `docs/PRD.md` scope/pilot gates; `docs/TRD.md` paths, ports, budgets and platform abstractions; `docs/API_SPEC.md` wire contracts; SQL migrations rather than historical schema counts; `docs/SECURITY.md` CA/DPAPI/revocation model; `docs/UI_UX_DESIGN.md` existing tokens/accessibility; `docs/IMPLEMENTATION_PLAN.md` M5/M6 release gates; `docs/WINDOWS_PRODUCT_TODO.md` validation still pending. Detailed screen behavior and migration procedure are independently specified in the two other Phase 4 documents, but this plan includes the architecture, boundaries, file map, sequence and checks needed to start implementation.
