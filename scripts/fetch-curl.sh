#!/usr/bin/env sh
set -eu
version="$(cat "$(dirname "$0")/../vendor/curl.version")"
archive="curl-${version}.tar.xz"
url="https://curl.se/download/${archive}"
root="$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)"
cd "$root/vendor"

curl -fL --proto '=https' --tlsv1.2 "$url" -o "$archive"
if grep -q FILL_FROM_OFFICIAL_RELEASE_METADATA curl.sha256; then
  echo "Refusing to continue: fill vendor/curl.sha256 from verified official metadata." >&2
  exit 1
fi
sha256sum -c curl.sha256
rm -rf curl
tar -xf "$archive"
mv "curl-${version}" curl
for patch_file in "$root/vendor/patches/curl"/*.patch; do
  [ -e "$patch_file" ] || continue
  patch -f -N -F 0 -p1 -d "$root/vendor/curl" < "$patch_file"
  printf 'Applied %s\n' "$(basename "$patch_file")"
done
printf 'Vendored curl %s\n' "$version"
