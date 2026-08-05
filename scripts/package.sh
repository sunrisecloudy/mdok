#!/usr/bin/env bash
set -euo pipefail

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
version=$(awk -F '"' '/^version = / { print $2; exit }' "$root/crates/mdok-cli/Cargo.toml")
target=${TARGET:-$(rustc -vV | awk '/host:/ { print $2 }')}
dist="$root/dist"
dist=${MDOK_DIST:-$dist}
archive_base="mdok-$version-$target"
mkdir -p "$dist"
stage=$(mktemp -d "$dist/.mdok-stage.XXXXXX")
source_state=$(mktemp "$dist/.mdok-source-state.XXXXXX")
source_tar=$(mktemp "$dist/.mdok-source-tar.XXXXXX")
signing_key=${MDOK_SIGNING_KEY:-}
public_key=${MDOK_SIGNING_PUBLIC_KEY:-$signing_key}
require_signature=${MDOK_REQUIRE_SIGNATURE:-0}
release_smoke=${MDOK_RELEASE_SMOKE:-0}
allow_dirty=${MDOK_ALLOW_DIRTY_RELEASE:-0}
cleanup() { rm -rf -- "$stage" "$source_state" "$source_tar"; }
trap cleanup EXIT HUP INT TERM

case "$require_signature:$release_smoke:$allow_dirty" in
  0:0:0|0:0:1|0:1:0|0:1:1|1:0:0|1:0:1|1:1:0|1:1:1) ;;
  *)
    echo "MDOK_REQUIRE_SIGNATURE, MDOK_RELEASE_SMOKE, and MDOK_ALLOW_DIRTY_RELEASE must be 0 or 1" >&2
    exit 2
    ;;
esac
if [ "$require_signature" = 1 ] || [ "$release_smoke" = 1 ]; then
  if [ -z "$signing_key" ]; then
    echo "Refusing release packaging: MDOK_SIGNING_KEY is required" >&2
    exit 2
  fi
fi
if [ -n "${MDOK_SIGNING_PUBLIC_KEY:-}" ] && [ -z "$signing_key" ]; then
  echo "Refusing release packaging: MDOK_SIGNING_PUBLIC_KEY requires MDOK_SIGNING_KEY" >&2
  exit 2
fi

python3 "$root/scripts/release_signing.py" source-state \
  --root "$root" \
  --output "$source_state"
dirty=$(python3 -c 'import json, sys; print(str(json.load(open(sys.argv[1]))["working_tree_dirty"]).lower())' "$source_state")
if [ -n "$signing_key" ] && [ "$dirty" = true ] && [ "$allow_dirty" != 1 ]; then
  echo "Refusing signed packaging from a dirty checkout; set MDOK_ALLOW_DIRTY_RELEASE=1 only for an explicit local exception" >&2
  exit 2
fi

cargo build --locked --manifest-path "$root/Cargo.toml" --release --package mdok-cli --target "$target"
binary="$root/target/$target/release/mdok"
binary_name=mdok
archive_suffix=.tar.gz
case "$target" in
  *-pc-windows-msvc)
    binary="$binary.exe"
    binary_name=mdok.exe
    archive_suffix=.zip
    ;;
esac
cp "$binary" "$stage/$binary_name"
cp "$root/LICENSE" "$root/THIRD_PARTY.md" "$root/vendor/curl/COPYING" "$stage/"
mkdir -p "$stage/patches/curl"
cp "$root/vendor/patches/curl"/*.patch "$stage/patches/curl/"
python3 "$root/scripts/generate_sbom.py" --output "$stage/mdok.spdx.json"

source_archive="$dist/mdok-$version-source.tar.gz"
git -C "$root" archive \
  --format=tar \
  --prefix="mdok-$version-source/" \
  HEAD > "$source_tar"
gzip -n < "$source_tar" > "$source_archive"
python3 - "$source_archive" "$version" <<'PY'
import pathlib
import sys
import tarfile

archive_path = pathlib.Path(sys.argv[1])
version = sys.argv[2]
prefix = f"mdok-{version}-source/"
try:
    with tarfile.open(archive_path, mode="r:gz") as archive:
        members = archive.getmembers()
except (OSError, tarfile.TarError) as error:
    raise SystemExit(f"source archive validation failed: {error}")

if not members:
    raise SystemExit("source archive validation failed: archive is empty")
for member in members:
    name = pathlib.PurePosixPath(member.name)
    if member.name.startswith("/") or ".." in name.parts or not member.name.startswith(prefix):
        raise SystemExit(f"source archive validation failed: unsafe member {member.name!r}")
PY

python3 "$root/scripts/generate_provenance.py" \
  --binary "$binary" \
  --target "$target" \
  --source-archive "$source_archive" \
  --source-state "$source_state" \
  --output "$stage/mdok.provenance.json"
python3 "$root/scripts/create_reproducible_archive.py" \
  --source "$stage" \
  --prefix "$archive_base" \
  --output "$dist/$archive_base$archive_suffix"

target_archive="$dist/$archive_base$archive_suffix"
python3 "$root/scripts/release_signing.py" checksums \
  --artifact "$target_archive" \
  --artifact "$source_archive"

if [ -n "$signing_key" ]; then
  manifest="$dist/$archive_base.release.json"
  python3 "$root/scripts/sign_release.py" \
    --key "$signing_key" \
    --manifest "$manifest" \
    --version "$version" \
    --target "$target" \
    --source-state "$source_state" \
    --artifact "$target_archive" \
    --artifact "$source_archive"
  python3 "$root/scripts/verify_release.py" \
    --key "$public_key" \
    --manifest "$manifest"
  if [ "$release_smoke" = 1 ]; then
    if [ "$allow_dirty" = 1 ]; then
      python3 "$root/scripts/release_smoke.py" \
        --key "$public_key" \
        --manifest "$manifest" \
        --allow-dirty
    else
      python3 "$root/scripts/release_smoke.py" \
        --key "$public_key" \
        --manifest "$manifest"
    fi
  fi
else
  printf 'Packaged unsigned local artifacts (set MDOK_SIGNING_KEY for signed output)\n'
fi
printf 'Packaged %s and %s\n' "$target_archive" "$source_archive"
