# Home Hub OS image builder (P4.1a)

This directory is a Buildroot external tree (`BR2_EXTERNAL`). Its `external.desc`,
`Config.in`, and `external.mk` register the Home Hub package without editing
Buildroot. `buildroot-config/` holds the x86-64 appliance and kernel fragments;
`board/homehub/` holds the GPT layout and GRUB menu; `scripts/` assembles an
image; `tests/` boots it in QEMU. The source checkout is used for the three
Rust binaries through `package/homehub/`.

## Pinned inputs and host

`sources.lock.json` records release URLs, tags, SHA-256 checksums and the
requested release lines. Buildroot is pinned to 2024.02.13, Linux to 6.6.77,
Mesa to 24.0.9, PipeWire to 1.0.9, WirePlumber to 0.4.17, WebKitGTK to
2.44.4, labwc to 0.7.4 and systemd to v255. The Rust workspace's `Cargo.lock`
is the dependency lock for Home Hub binaries. Buildroot's internal toolchain
uses glibc; the init system is systemd. The defconfig enables EFI GRUB,
Mesa/DRM/EGL/GLES, PipeWire/WirePlumber, NetworkManager, labwc and WebKitGTK.

Use an x86-64 Linux build host with GNU make, gcc/g++, Python 3, curl, tar,
sha256sum, genimage, mke2fs, grub-editenv and QEMU. Buildroot needs its normal
host development tools and substantial disk space. The builder checks required
tools before use. Use a clean checkout and a dedicated `BUILDROOT_DIR` or let
the script download and verify the pinned Buildroot archive.

**Dependency gate:** stock Buildroot 2024.02.13 does not resolve all requested
release lines: it carries PipeWire 0.3.81, WirePlumber 0.4.8, WebKitGTK 2.46.4
and systemd 254.13, and has no labwc package. The builder checks the actual
package definitions and stops before compiling when they disagree with the
lock manifest. The Buildroot package backports or a revised base release must
be chosen and recorded before a release image can be produced. A successful
QEMU boot alone also does not satisfy the WebKitGTK runtime gate in the phase
plan. Do not treat the manifest as an applied Buildroot override.

## Build and layout

After resolving that dependency gate, provide a signed 2 GiB recovery
partition image in `RECOVERY_IMAGE` and run:

```sh
RECOVERY_IMAGE=/path/to/signed-recovery.img \
  tools/image-builder/scripts/build-image.sh
tools/image-builder/tests/boot-smoke.sh
```

The script verifies the Buildroot archive hash, configures with
`BR2_EXTERNAL=<this directory>`, builds the kernel and rootfs, places the
kernel and a per-slot GRUB config on the ESP, generates a 16 GiB ext4 data
image, and uses `genimage` to produce `tools/image-builder/target/homehub-os.img`.
It checks that each squashfs root fits within its 6 GiB slot with 20% spare.
The GPT image has a 512 MiB FAT32 ESP, two 6 GiB read-only squashfs root
slots, 2 GiB recovery slot, and 16 GiB writable Data partition. The image is
UEFI-only for this slice; BIOS embedding and the Keep Windows installer are
later work. No signing keys are bundled here.

GRUB stores `active_slot`, `previous_slot`, and `boot_counter` in its ESP
environment. It falls back after three unconfirmed boots. A future health
service must reset the counter after its full readiness checks; without that
service, the fallback is a safety mechanism, not an update protocol. The
recovery image must have its own verified boot chain. Data mount, root
integrity verification, firmware selection, SBOM/license export, and RAUC
signing remain release gates outside this slice.

The QEMU smoke test uses UEFI firmware through `OVMF_CODE` and records the
serial console under `tools/image-builder/target/`. It checks for kernel and
systemd startup plus a login prompt; it does not assert graphics, pairing,
or physical-device compatibility.
