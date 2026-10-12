#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: update-bundle.sh --rootfs PATH --version VERSION --output PATH [options]
  --cert PATH       PEM signing certificate (required unless --dry-run)
  --key PATH        Matching PEM private key (required unless --dry-run)
  --compatible TAG  Target compatible tag (default: homehub-x86_64)
  --format FORMAT   Bundle format: verity or plain (default: verity)
  --dry-run         Validate inputs and print the synthesized manifest
  --help, -h        Show this help
EOF
}

fatal() { printf 'Error: %s\n' "$*" >&2; exit 1; }

rootfs='' version='' cert='' key='' output=''
compatible='homehub-x86_64' format='verity' dry_run=0
while (($#)); do
  case "$1" in
    --rootfs|--version|--cert|--key|--output|--compatible|--format)
      (($# >= 2)) || fatal "Missing value for $1"
      case "$1" in
        --rootfs) rootfs=$2 ;;
        --version) version=$2 ;;
        --cert) cert=$2 ;;
        --key) key=$2 ;;
        --output) output=$2 ;;
        --compatible) compatible=$2 ;;
        --format) format=$2 ;;
      esac
      shift 2 ;;
    --dry-run) dry_run=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fatal "Unknown argument: $1" ;;
  esac
done

[[ -n $rootfs ]] || fatal 'Missing --rootfs'
[[ -f $rootfs && -r $rootfs ]] || fatal "Rootfs is missing or unreadable: $rootfs"
[[ -n $version ]] || fatal 'Missing --version'
[[ -n $output ]] || fatal 'Missing --output'
[[ $format == verity || $format == plain ]] || fatal 'Format must be verity or plain'
[[ $version =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ ]] || fatal 'Invalid version'
[[ $compatible =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ ]] || fatal 'Invalid compatible tag'
rootfs_name=${rootfs##*/}
[[ $rootfs_name != manifest.raucm && $rootfs_name != *$'\n'* ]] || fatal 'Invalid rootfs filename'

if (( ! dry_run )); then
  [[ -n $cert && -f $cert && -r $cert ]] || fatal 'Certificate is missing or unreadable'
  [[ -n $key && -f $key && -r $key ]] || fatal 'Private key is missing or unreadable'
fi

if command -v sha256sum >/dev/null 2>&1; then
  rootfs_sha256=$(sha256sum "$rootfs" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
  rootfs_sha256=$(shasum -a 256 "$rootfs" | awk '{print $1}')
else
  fatal 'sha256sum or shasum is required'
fi
rootfs_size=$(wc -c < "$rootfs" | tr -d '[:space:]')
[[ $rootfs_sha256 =~ ^[[:xdigit:]]{64}$ && $rootfs_size =~ ^[0-9]+$ ]] || fatal 'Could not inspect rootfs'

manifest=$(cat <<EOF
[update]
compatible=$compatible
version=$version

[bundle]
format=$format

[image.rootfs]
filename=$rootfs_name
sha256=$rootfs_sha256
size=$rootfs_size
EOF
)

if (( dry_run )); then
  printf '%s\n' "$manifest"
  printf 'Dry run: manifest validated successfully; would bundle %s bytes.\n' "$rootfs_size" >&2
  exit 0
fi

bundle_dir=$(mktemp -d "${TMPDIR:-/tmp}/homehub-rauc.XXXXXX")
trap 'rm -rf "$bundle_dir"' EXIT
printf '%s\n' "$manifest" > "$bundle_dir/manifest.raucm"
cp "$rootfs" "$bundle_dir/$rootfs_name"

if command -v rauc >/dev/null 2>&1; then
  rauc bundle --cert="$cert" --key="$key" "$bundle_dir" "$output"
else
  printf 'Warning: rauc is unavailable; writing an unsigned mock tar archive. A signed RAUC bundle requires rauc on Linux.\n' >&2
  tar -cf "$output" -C "$bundle_dir" manifest.raucm "$rootfs_name"
fi

printf 'Bundle: %s\nRootfs size: %s bytes\nRootfs SHA-256: %s\nCompatible: %s\nVersion: %s\n' \
  "$output" "$rootfs_size" "$rootfs_sha256" "$compatible" "$version"
