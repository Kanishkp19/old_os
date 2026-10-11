# P4.S: Week 1 Wayland/WebKit VM spike

This disposable harness exercises Gate G0 in `docs/PHASE4_IMPLEMENTATION_PLAN.md` §2.3.1. Run it on an x86_64 Linux build host with Buildroot dependencies, Rust, Node/npm, pkg-config, WebKitGTK/GTK development libraries, `jq`, and QEMU. Build output stays in `target/`; do not commit images, credentials, or logs. No host libraries are mounted into the guest.

## Five working days

1. **Day 1:** Review `dependencies.lock.json`, install the exact toolchain and package sources, save Cargo/npm lockfiles generated from the pinned manifests, and start `./build.sh --compositor=labwc`. Keep `target/build-labwc.log`. Missing Buildroot packages or incompatible ABI is a failed inventory item, not a substitute package choice.
2. **Day 2:** Boot with `./run-qemu.sh --compositor=labwc`. Supply an authenticated, guest-local hub proxy at `127.0.0.1:47801` and use the app's **Read overview** button. Save the image SHA-256, `target/qemu-serial-labwc.log`, screenshot, and a redacted proof of the authenticated response. Host port 47801 forwards to the guest for diagnostics; the app connects only to guest loopback. Never copy a token into `results.json`.
3. **Day 3:** Run `./build.sh --compositor=smithay` and boot the same Buildroot configuration with `./run-qemu.sh --compositor=smithay`. Record keyboard and Hindi IME, clipboard, popup/focus, resize/scale, and close/reopen separately. State whether the run used guest DRM or a nested compositor. The included Smithay program exposes a diagnostic socket and initial xdg configure; it has no renderer or physical input backend yet. Its launch cannot pass the Day 3 interaction cases. Record these as blockers until a real guest backend is attached.
4. **Day 4:** On labwc test a GTK4 layer-shell panel, AT-SPI control discovery, lock/capture protocol availability, guest network/audio, and two restarts. Capture `/proc/*/smaps_rollup` PSS/RSS and whole-system `/proc/meminfo` after idle settles. Record any software rendering limitation.
5. **Day 5:** Rebuild from saved inputs, repeat the guest app/proxy read, then run `./smoke.sh --day=5`. Fill the evidence fields in `results.json` and make the G0 decision in `DECISION.md`. Every missing required observation remains pending or blocked.

`./smoke.sh --day=N` performs safe checks that can be automated and writes `results.json`; manual guest observations are supplied as `--evidence=path/to/evidence.json` containing case IDs and `pass`/`fail` plus an artifact path. `./smoke.sh --day=all` does not turn unobserved cases into passes. The result schema and case IDs are in `results.json`.

## Image comparison

`build.sh` uses one pinned Buildroot tree and one generated defconfig. It builds a root filesystem plus kernel and emits `target/homehub-spike.img`, a raw ext4 disk image booted by QEMU's direct kernel loader. The selected compositor is copied into the overlay at `/usr/local/bin/homehub-compositor`; app and dependency package configuration remain the same. Save each image hash before rebuilding, since the next build replaces the target image. Buildroot package availability and guest execution must be checked; the script stops rather than silently falling back to host libraries.

Environment inputs: `BUILDROOT_DIR` (checked-out pinned Buildroot release), `TAURI_BINARY` and `SMITHAY_BINARY` (Linux x86_64 binaries if already cross-compiled), and `RUST_TARGET` (default `x86_64-unknown-linux-gnu`). The build script can compile both Rust binaries on an x86_64 Linux host with matching GTK/WebKit dependencies when binary paths are omitted. Enter the proxy token in the guest app at test time; the image does not store it. The proxy itself is deliberately not fabricated by this harness: the required authenticated read must reach the actual service under test.

`run-qemu.sh` boots with 2 GiB RAM, two vCPUs, virtio-gpu, user networking, and host 47801 to guest 47801 forwarding. The VM is disposable. The raw image uses direct kernel boot and has no installed bootloader; it is bootable through this QEMU command, not independently via BIOS/UEFI.

Gate rule: Buildroot + labwc + packaged Tauri/WebKitGTK must work **inside** the guest to accept the base. Smithay remains optional even when its cases pass. If the VM or toolchain is unavailable, record a blocked gate and leave baseline confirmation pending.
