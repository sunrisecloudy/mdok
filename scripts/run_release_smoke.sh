#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
temp_root=$(mktemp -d "${TMPDIR:-/tmp}/mdok-release-smoke.XXXXXX")
cleanup() {
    rm -rf -- "$temp_root"
}
trap cleanup EXIT HUP INT TERM
umask 077

key="$temp_root/test-signing-key.pem"
public_key="$temp_root/test-signing-key.pub.pem"
dist="$temp_root/dist"

openssl genpkey -algorithm ED25519 -out "$key" >/dev/null 2>&1
openssl pkey -in "$key" -pubout -out "$public_key" >/dev/null 2>&1

MDOK_DIST="$dist" \
MDOK_SIGNING_KEY="$key" \
MDOK_SIGNING_PUBLIC_KEY="$public_key" \
MDOK_REQUIRE_SIGNATURE=1 \
MDOK_RELEASE_SMOKE=1 \
    bash "$root/scripts/package.sh"

printf 'Ephemeral-key signed release smoke passed; test key was removed\n'
