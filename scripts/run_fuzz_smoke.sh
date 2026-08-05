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

if cargo fuzz --version >/dev/null 2>&1; then
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
else
    echo "cargo-fuzz unavailable; running deterministic parser-boundary fallback"
    MDOK_FUZZ_RUNS="$runs" MDOK_FUZZ_MAX_LEN="$max_len" \
        cargo test --manifest-path "$root/fuzz/Cargo.toml" --locked \
            --test smoke -- --nocapture
    echo "Fuzz smoke fallback completed: markdown shell_template curl_ffi"
fi
