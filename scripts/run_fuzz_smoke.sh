#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
runs=${MDOK_FUZZ_RUNS:-128}
max_len=${MDOK_FUZZ_MAX_LEN:-4096}
timeout=${MDOK_FUZZ_TIMEOUT:-5}
sanitizer=${MDOK_FUZZ_SANITIZER:-address}

case "$runs" in
    ''|*[!0-9]*) echo "run_fuzz_smoke: MDOK_FUZZ_RUNS must be numeric" >&2; exit 2 ;;
esac
case "$max_len" in
    ''|*[!0-9]*) echo "run_fuzz_smoke: MDOK_FUZZ_MAX_LEN must be numeric" >&2; exit 2 ;;
esac
case "$timeout" in
    ''|*[!0-9]*) echo "run_fuzz_smoke: MDOK_FUZZ_TIMEOUT must be numeric" >&2; exit 2 ;;
esac

if ! cargo fuzz --version >/dev/null 2>&1; then
    echo "run_fuzz_smoke: cargo-fuzz is required (install with: cargo install cargo-fuzz)" >&2
    exit 1
fi

cd "$root/fuzz"
for target in markdown shell_template curl_ffi; do
    echo "==> fuzz target: $target"
    cargo fuzz run --sanitizer "$sanitizer" "$target" -- \
        -runs="$runs" \
        -max_len="$max_len" \
        -timeout="$timeout" \
        -rss_limit_mb=1024 \
        -seed=1 \
        -print_final_stats=1
done

echo "Fuzz smoke completed: markdown shell_template curl_ffi"
