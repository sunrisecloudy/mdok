#!/usr/bin/env sh
set -eu

usage() {
    cat <<'EOF'
Usage: scripts/run_sanitizers.sh [asan|ubsan|lsan|tsan|all]

Builds the vendored C bridge in an isolated target directory and runs the
focused native FFI safety suite. Markdown, shell, and template inputs are
covered by the cargo-fuzz targets and their normal Rust tests.
The script is intentionally local tooling; CI orchestration belongs elsewhere.

Environment:
  MDOK_SANITIZER_TARGET_DIR  Override target/sanitizers/<mode>
  CC                         C compiler (defaults to clang)
  RUSTFLAGS                  Additional Rust flags, preserved and extended
  CFLAGS / LDFLAGS           Additional native flags, preserved and extended
EOF
}

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
mode=${1:-all}

case "$mode" in
    asan|ubsan|lsan|tsan|all) ;;
    -h|--help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        exit 2
        ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
    echo "run_sanitizers: cargo is required" >&2
    exit 1
fi

if [ -z "${CC:-}" ]; then
    CC=clang
fi
if ! command -v "$CC" >/dev/null 2>&1; then
    echo "run_sanitizers: C compiler not found: $CC" >&2
    exit 1
fi
if ! command -v cmake >/dev/null 2>&1; then
    echo "run_sanitizers: cmake is required to build the native bridge" >&2
    exit 1
fi

base_cflags=${CFLAGS:-}
base_ldflags=${LDFLAGS:-}
base_rustflags=${RUSTFLAGS:-}
base_dyld_library_path=${DYLD_LIBRARY_PATH:-}
base_dyld_insert_libraries=${DYLD_INSERT_LIBRARIES:-}
base_ld_library_path=${LD_LIBRARY_PATH:-}

check_sanitizer() {
    sanitizer=$1
    flag=$2
    probe_dir=$(mktemp -d "${TMPDIR:-/tmp}/mdok-sanitizer.XXXXXX") || {
        echo "run_sanitizers: cannot create a temporary directory for $sanitizer" >&2
        return 1
    }
    probe="$probe_dir/probe"
    compiler_log="$probe_dir/compiler.log"
    if ! printf '%s\n' 'int main(void) { return 0; }' | \
        "$CC" -x c - -fno-omit-frame-pointer -fsanitize="$flag" -o "$probe" \
        2>"$compiler_log"; then
        echo "run_sanitizers: $sanitizer is unavailable; $CC cannot link -fsanitize=$flag" >&2
        sed -n '1,12p' "$compiler_log" >&2
        rm -rf -- "$probe_dir"
        return 1
    fi
    rm -rf -- "$probe_dir"
}

sanitizer_runtime_arg() {
    sanitizer=$1
    flag=$2
    case "$(uname -s)" in
        Darwin)
            case "$flag" in
                address) runtime_prefix=asan ;;
                undefined) runtime_prefix=ubsan ;;
                leak) runtime_prefix=lsan ;;
                thread) runtime_prefix=tsan ;;
            esac
            runtime=$($CC -print-file-name="libclang_rt.${runtime_prefix}_osx_dynamic.dylib" 2>/dev/null || true)
            if [ ! -f "$runtime" ]; then
                echo "run_sanitizers: $sanitizer is unavailable; Apple Clang runtime for $flag was not found" >&2
                return 1
            fi
            printf '%s' "$runtime"
            ;;
        Linux)
            runtime_dir=$($CC --print-runtime-dir 2>/dev/null || true)
            case "$flag" in
                address) runtime_prefix=asan ;;
                undefined) runtime_prefix=ubsan_standalone ;;
                leak) runtime_prefix=lsan ;;
                thread) runtime_prefix=tsan ;;
            esac
            runtime=$(find "$runtime_dir" -maxdepth 1 -type f \
                -name "libclang_rt.${runtime_prefix}-*.a" -print -quit 2>/dev/null || true)
            if [ ! -f "$runtime" ]; then
                echo "run_sanitizers: $sanitizer is unavailable; Clang runtime for $flag was not found" >&2
                return 1
            fi
            printf '%s' "$runtime"
            ;;
        *)
            echo "run_sanitizers: unsupported host for sanitizer smoke: $(uname -s)" >&2
            return 1
            ;;
    esac
}

run_one() {
    sanitizer=$1
    case "$sanitizer" in
        asan) flag=address ;;
        ubsan) flag=undefined ;;
        lsan) flag=leak ;;
        tsan) flag=thread ;;
    esac

    check_sanitizer "$sanitizer" "$flag"
    runtime=$(sanitizer_runtime_arg "$sanitizer" "$flag")
    runtime_dir=$(dirname "$runtime")

    target_root=${MDOK_SANITIZER_TARGET_DIR:-"$root/target/sanitizers"}
    target_dir="$target_root/$sanitizer"
    mkdir -p "$target_dir"

    echo "==> sanitizer: $sanitizer (target: $target_dir)"
    export CC
    export CARGO_TARGET_DIR="$target_dir"
    export CFLAGS="$base_cflags -fsanitize=$flag -fno-omit-frame-pointer"
    export LDFLAGS="$base_ldflags -fsanitize=$flag"
    export RUSTFLAGS="$base_rustflags -C debuginfo=1 -C link-arg=-fsanitize=$flag -C link-arg=$runtime"
    case "$(uname -s)" in
        Darwin)
            export DYLD_LIBRARY_PATH="$runtime_dir${base_dyld_library_path:+:$base_dyld_library_path}"
            export DYLD_INSERT_LIBRARIES="$runtime${base_dyld_insert_libraries:+:$base_dyld_insert_libraries}"
            ;;
        Linux) export LD_LIBRARY_PATH="$runtime_dir${base_ld_library_path:+:$base_ld_library_path}" ;;
    esac

    case "$sanitizer" in
        asan)
            case "$(uname -s)" in
                Darwin) asan_defaults=halt_on_error=1:allocator_may_return_null=1 ;;
                *) asan_defaults=detect_leaks=1:halt_on_error=1:allocator_may_return_null=1 ;;
            esac
            export ASAN_OPTIONS="${ASAN_OPTIONS:-$asan_defaults}"
            ;;
        ubsan)
            export UBSAN_OPTIONS="${UBSAN_OPTIONS:-halt_on_error=1:print_stacktrace=1}"
            ;;
        lsan)
            export LSAN_OPTIONS="${LSAN_OPTIONS:-halt_on_error=1:detect_leaks=1}"
            ;;
        tsan)
            export TSAN_OPTIONS="${TSAN_OPTIONS:-halt_on_error=1:second_deadlock_stack=1}"
            ;;
    esac

    cargo test --locked -p mdok-curl-sys --test bridge_safety -- --test-threads=1
}

if [ "$mode" = all ]; then
    for sanitizer in asan ubsan lsan tsan; do
        run_one "$sanitizer"
    done
else
    run_one "$mode"
fi

echo "Sanitizer smoke completed: $mode"
