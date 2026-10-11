#!/usr/bin/env bash
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
target_dir="$here/target"
compositor=
for arg in "$@"; do
  case "$arg" in
    --compositor=labwc|--compositor=smithay) compositor=${arg#*=} ;;
    *) echo "usage: $0 --compositor=labwc|smithay" >&2; exit 2 ;;
  esac
done
[[ -n "$compositor" ]] || { echo 'select a compositor' >&2; exit 2; }
[[ $(uname -s) == Linux && $(uname -m) == x86_64 ]] || { echo 'x86_64 Linux build host required' >&2; exit 2; }
for tool in jq cargo rustc node npm make file sha256sum; do command -v "$tool" >/dev/null || { echo "missing $tool" >&2; exit 2; }; done
rust_version=$(rustc --version | awk '{print $2}')
node_version=$(node -p 'process.versions.node')
[[ "$rust_version" == "$(jq -r '.rust.version' "$here/dependencies.lock.json")" ]] || { echo "Rust pin mismatch: $rust_version" >&2; exit 2; }
[[ "$node_version" == "$(jq -r '.node.version' "$here/dependencies.lock.json")" ]] || { echo "Node pin mismatch: $node_version" >&2; exit 2; }
buildroot=${BUILDROOT_DIR:-}
[[ -n "$buildroot" && -f "$buildroot/Makefile" ]] || { echo 'BUILDROOT_DIR must name a checked-out Buildroot tree' >&2; exit 2; }
expected=$(jq -r '.buildroot.version' "$here/dependencies.lock.json")
actual=$(make -s -C "$buildroot" print-version 2>/dev/null || true)
[[ "$actual" == "$expected" ]] || { echo "Buildroot $expected required; found ${actual:-unknown}" >&2; exit 2; }

mkdir -p "$target_dir"
overlay="$target_dir/overlay-$compositor"
mkdir -p "$overlay/usr/local/bin" "$overlay/etc/profile.d" "$overlay/etc/xdg/labwc" "$overlay/root"
app_bin=${TAURI_BINARY:-}
if [[ -z "$app_bin" ]]; then
  export npm_config_cache="$target_dir/npm-cache"
  npm --prefix "$here/tauri-smoke" ci
  printf 'fn main() { tauri_build::build(); }\n' > "$here/tauri-smoke/src-tauri/build.rs"
  npm --prefix "$here/tauri-smoke" run tauri -- build
  app_bin="$here/tauri-smoke/src-tauri/target/release/tauri-smoke"
fi
[[ -f "$app_bin" ]] || { echo "missing Tauri binary: $app_bin" >&2; exit 2; }
file "$app_bin" | grep -q 'ELF 64-bit.*x86-64' || { echo 'Tauri binary must be Linux x86_64 ELF' >&2; exit 2; }
install -m 0755 "$app_bin" "$overlay/usr/local/bin/tauri-smoke"

if [[ "$compositor" == smithay ]]; then
  compositor_bin=${SMITHAY_BINARY:-}
  if [[ -z "$compositor_bin" ]]; then
    cargo build --manifest-path "$here/smithay-harness/Cargo.toml" --release --target "${RUST_TARGET:-x86_64-unknown-linux-gnu}"
    compositor_bin="$here/smithay-harness/target/${RUST_TARGET:-x86_64-unknown-linux-gnu}/release/smithay-harness"
  fi
  [[ -f "$compositor_bin" ]] || { echo "missing Smithay binary: $compositor_bin" >&2; exit 2; }
  file "$compositor_bin" | grep -q 'ELF 64-bit.*x86-64' || { echo 'Smithay binary must be Linux x86_64 ELF' >&2; exit 2; }
  install -m 0755 "$compositor_bin" "$overlay/usr/local/bin/homehub-compositor"
else
  # labwc comes from the guest package; the session selects it explicitly.
  cat > "$overlay/etc/xdg/labwc/autostart" <<'EOF'
/usr/local/bin/tauri-smoke &
EOF
fi

cat > "$overlay/etc/profile.d/homehub-spike.sh" <<'EOF'
export XDG_RUNTIME_DIR=/run/user/1000
export GDK_BACKEND=wayland
EOF
cat > "$overlay/root/START-HERE" <<EOF
Compositor: $compositor
Launch manually in a logged-in Wayland session. Record app startup, input and proxy evidence.
The image has no fabricated hub proxy or authentication token.
EOF

out="$target_dir/buildroot-$compositor"
mkdir -p "$out"
make -C "$buildroot" O="$out" qemu_x86_64_defconfig
config="$out/.config"
set_config() {
  local key=$1 value=$2
  sed -i "/^${key}=/d; /^# ${key} is not set/d" "$config"
  printf '%s=%s\n' "$key" "$value" >> "$config"
}
set_config BR2_TOOLCHAIN_BUILDROOT_GLIBC y
set_config BR2_INIT_SYSTEMD y
set_config BR2_PACKAGE_LABWC y
set_config BR2_PACKAGE_WEBKITGTK y
set_config BR2_PACKAGE_GTK3 y
set_config BR2_PACKAGE_GTK4 y
set_config BR2_PACKAGE_MESA3D y
set_config BR2_PACKAGE_WAYLAND y
set_config BR2_TARGET_ROOTFS_EXT2 y
set_config BR2_TARGET_ROOTFS_EXT2_4 y
set_config BR2_ROOTFS_OVERLAY "\"$overlay\""
make -C "$buildroot" O="$out" olddefconfig
for key in BR2_TOOLCHAIN_BUILDROOT_GLIBC BR2_INIT_SYSTEMD BR2_PACKAGE_LABWC BR2_PACKAGE_WEBKITGTK BR2_PACKAGE_GTK3 BR2_PACKAGE_GTK4 BR2_PACKAGE_MESA3D BR2_PACKAGE_WAYLAND; do
  grep -qx "$key=y" "$config" || { echo "Buildroot rejected $key; record as Day 1 packaging blocker" >&2; exit 1; }
done
make -C "$buildroot" O="$out" "-j${JOBS:-2}" 2>&1 | tee "$target_dir/build-$compositor.log"
rootfs="$out/images/rootfs.ext2"
kernel="$out/images/bzImage"
[[ -s "$rootfs" && -s "$kernel" ]] || { echo 'Buildroot did not produce rootfs and kernel' >&2; exit 1; }
cp "$rootfs" "$target_dir/homehub-spike.img"
cp "$kernel" "$target_dir/bzImage"
sha256sum "$target_dir/homehub-spike.img" | tee "$target_dir/homehub-spike-$compositor.sha256"
printf '%s\n' "$compositor" > "$target_dir/image-compositor"
echo "Built $target_dir/homehub-spike.img (raw ext4 rootfs, QEMU direct-kernel boot)"
