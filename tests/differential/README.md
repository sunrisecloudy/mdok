# Curl differential and option-conformance tests

`scripts/run_curl_differential.py` builds the pinned curl source in
`vendor/curl` as a standalone executable, then drives two parser/planner
comparisons:

1. Each canonical curl option and short alias is passed to the bundled curl
   executable with `--help` as the terminal action. This exercises curl's
   argument parser without opening a connection.
2. The same option is rendered into a generated one-step Markdown document and
   passed to `mdok plan --json`. The report records the planned/rejected state,
   diagnostic codes, and the normalized curl parser error kind.

The harness covers all long options in `vendor/curl/src/tool_listhelp.c`, all
short aliases found there, every explicit row in
`specs/curl-option-policy.csv`, missing-argument probes for value-taking
options, repeated value-taking options (including their aliases), curl
`--no-` negated forms, five transfer-characteristic combinations, and an
unknown-option sentinel. Policy-gated file cases deliberately
use a temporary path outside the configured read roots so the policy gate is
exercised during planning.

Run the complete suite from the repository root:

```sh
python3 scripts/run_curl_differential.py \
  --report /tmp/mdok-curl-differential.json
```

The first run configures and builds curl under the ignored
`target/differential/curl-build` directory. Use `--curl PATH` to reuse an
already-built bundled executable, `--limit N` for a quick smoke run, and
`--no-strict` when collecting a report while known conformance mismatches are
being repaired. The default CMake boundary is pinned to the version in
`vendor/curl.version`; an explicitly supplied executable must report that
same version or the harness fails before generating cases. Optional curl
features unavailable in the isolated build are recorded as
`feature-unavailable`. The harness never executes a transfer and does not
modify native runtime, CI, fuzz/benchmark, or release-signing files.
