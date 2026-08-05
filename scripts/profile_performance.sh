#!/usr/bin/env sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
output=${MDOK_PROFILE_OUTPUT:-$root/target/performance-profile}
bench_args=${MDOK_PROFILE_BENCH_ARGS:-}

mkdir -p "$output"
cd "$root"

run_capture() {
    name=$1
    shift
    {
        printf '$'
        printf ' %s' "$@"
        printf '\n'
        "$@"
    } >"$output/$name.txt" 2>&1
}

run_optional() {
    name=$1
    executable=$2
    shift 2
    if command -v "$executable" >/dev/null 2>&1; then
        run_capture "$name" "$executable" "$@"
        return 0
    fi
    printf 'not installed: %s\n' "$executable" >"$output/$name.txt"
    return 0
}

run_capture git-source git rev-parse HEAD
run_capture git-status git status --short --branch
run_capture rustc rustc -vV
run_capture cargo cargo -V
run_capture uname uname -a

if command -v sysctl >/dev/null 2>&1; then
    run_capture system-cpu sysctl -n hw.model hw.logicalcpu hw.memsize
elif command -v lscpu >/dev/null 2>&1; then
    run_capture system-cpu lscpu
else
    printf 'CPU and RAM metadata unavailable on this host\n' >"$output/system-cpu.txt"
fi

bench_command="cargo bench --locked -p mdok-benchmarks --bench prd"
if [ -n "$bench_args" ]; then
    bench_command="$bench_command -- $bench_args"
fi
{
    printf '$ %s\n' "$bench_command"
    # shellcheck disable=SC2086
    sh -c "$bench_command"
} >"$output/criterion.txt" 2>&1

run_optional llvm-lines cargo llvm-lines --workspace --release
run_optional bloat cargo bloat --release --workspace --crates
run_optional samply samply record --output "$output/samply.json" -- cargo bench --locked -p mdok-benchmarks --bench prd
run_optional perf perf stat -d cargo bench --locked -p mdok-benchmarks --bench prd
run_optional heaptrack heaptrack --output "$output/heaptrack" cargo bench --locked -p mdok-benchmarks --bench prd
run_optional massif valgrind --tool=massif --massif-out-file="$output/massif.out" cargo bench --locked -p mdok-benchmarks --bench prd

printf 'Performance profiling artifacts written to %s\n' "$output"
