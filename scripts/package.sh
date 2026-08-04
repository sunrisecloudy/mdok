#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
version=$(awk -F '"' '/^version = / { print $2; exit }' "$root/crates/mdok-cli/Cargo.toml")
target=${TARGET:-$(rustc -vV | awk '/host:/ { print $2 }')}
dist="$root/dist"
archive_base="mdok-$version-$target"
mkdir -p "$dist"
stage=$(mktemp -d "$dist/.mdok-stage.XXXXXX")
cleanup() { rm -rf -- "$stage"; }
trap cleanup EXIT HUP INT TERM

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
python3 "$root/scripts/generate_provenance.py" \
  --binary "$binary" \
  --target "$target" \
  --output "$stage/mdok.provenance.json"
python3 "$root/scripts/create_reproducible_archive.py" \
  --source "$stage" \
  --prefix "$archive_base" \
  --output "$dist/$archive_base$archive_suffix"
shasum -a 256 "$dist/$archive_base$archive_suffix" > "$dist/$archive_base$archive_suffix.sha256"

source_archive="$dist/mdok-$version-source.tar.gz"
git -C "$root" archive --format=tar --prefix="mdok-$version-source/" HEAD | gzip -n > "$source_archive"
shasum -a 256 "$source_archive" > "$source_archive.sha256"
printf 'Packaged %s and %s\n' "$dist/$archive_base$archive_suffix" "$source_archive"
