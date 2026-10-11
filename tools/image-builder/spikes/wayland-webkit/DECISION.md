# Gate G0 decision record

Status: **PENDING — no guest evidence collected**  
Owner: unassigned  
Date/image commit: pending  
Selected baseline: pending; labwc is the plan's provisional baseline only.

| Required score | Result | Evidence |
|---|---|---|
| Buildroot glibc/systemd packaging, exact dependency inventory | Pending | `results.json` day 1 and build log |
| labwc guest boot and packaged Tauri/WebKitGTK startup | Pending | Image SHA-256, serial log, screenshot |
| Authenticated guest-loopback overview read | Pending | Redacted response and latency |
| Smithay guest or nested mode, keyboard/IME/clipboard/popup/focus/resize/scale/reopen | Pending | Per-case results and crash logs |
| GTK4 layer-shell, AT-SPI, lock/capture, network/audio, two restarts | Pending | Day 4 protocol and test artifacts |
| Idle whole-system memory, process RSS/PSS and CPU | Pending | Timestamped `/proc` samples |
| Rebuild and repeat guest smoke | Pending | Day 5 logs and image digest |
| Final Buildroot+labwc baseline confirmation | Pending | Requires every mandatory guest chain item above |

Decision rule: accept Buildroot only after the labwc/Tauri chain passes in the generated guest image. Smithay failure or missing evidence selects labwc for compositor planning but cannot itself prove labwc. If labwc works and Buildroot packaging fails, stop base-image commitment and request a separate Debian-minimal spike. If neither chain works, stop dependent shell/app scheduling. A missing VM/toolchain blocks G0. A funded 12–24 week custom-compositor track requires a separate decision even after Smithay passes.

Blockers: QEMU is absent from the current macOS host; guest evidence has not been collected. The Smithay harness currently validates protocol setup and initial xdg configure only; a renderer and physical input backend are still required for the Day 3 matrix. Re-evaluate on an x86_64 Linux build host. Revised estimate: pending measured dependency-chain outcome.
