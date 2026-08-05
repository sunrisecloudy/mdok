#!/usr/bin/env python3
"""Produce a local, repeatable dependency audit for the MDOK workspace.

The audit intentionally uses only Cargo's local metadata/tree commands and
Python's standard library.  It does not update the lockfile, resolve versions,
or contact a registry.  The generated Markdown is evidence for future
dependency-reduction work; it is not permission to remove a compatibility
dependency without running the differential and release checks.
"""

from __future__ import annotations

import argparse
import collections
import os
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable


SCRIPT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = SCRIPT_ROOT / "docs" / "DEPENDENCY_AUDIT.md"

WORKSPACE_RUNTIME_PACKAGES = {
    "mdok-cli",
    "mdok-core",
    "mdok-curl",
    "mdok-curl-sys",
    "mdok-jmespath",
    "mdok-markdown",
    "mdok-report",
    "mdok-runtime",
    "mdok-shell",
    "mdok-template",
}
BENCHMARK_PACKAGES = {"mdok-benchmarks"}
FIXTURE_PACKAGES = {"mdok-test-server"}
FUZZ_PACKAGES = {"mdok-fuzz"}

USAGE_NOTES = {
    "anyhow": "CLI and fixture-server application error context.",
    "base64": "Template/curl binary-body encoding and fixture-server payloads.",
    "clap": "Typed command-line parsing for `mdok` and the local test server.",
    "comrak": "CommonMark/GFM parsing, AST traversal, and source positions.",
    "criterion": "Benchmark harness and HTML reports; benchmark-only.",
    "cmake": "Build-time configuration for the native bundled libcurl bridge.",
    "flate2": "Fixture-server gzip/deflate responses; not required by the shipped CLI.",
    "jmespath": "JMESPath compilation/evaluation behind the MDOK expression API.",
    "percent-encoding": "Curl query/form encoding and template URL encoding.",
    "reqwest": "Blocking compatibility HTTP adapter for curl options outside the native safe subset.",
    "rustls": "TLS support for the local HTTPS fixture server.",
    "rustls-pemfile": "Fixture-server certificate/key PEM loading.",
    "rustls-pki-types": "Typed certificate/key inputs for the fixture server.",
    "serde": "Derive-based configuration, plan, report, and protocol serialization.",
    "serde_json": "JSON context, captures, reports, request bodies, and fixture payloads.",
    "sha2": "Fixture-server response digest support.",
    "tempfile": "Bounded response-body spill files and report output staging.",
    "thiserror": "Typed library error enums with stable MDOK error codes/messages.",
    "toml": "CLI configuration and Markdown TOML fence parsing.",
    "url": "URL parsing, query construction, and policy validation.",
    "walkdir": "Recursive CLI discovery of Markdown documents.",
}

INTENTIONAL_COMPATIBILITY = {
    "reqwest": "Intentional fallback adapter: native libcurl is limited to the policy-safe subset; reqwest preserves broader curl compatibility.",
    "rustls": "Intentional fixture-only TLS dependency; keep out of the CLI package if the fixture server is split later.",
    "rustls-pemfile": "Intentional fixture-only certificate loader.",
    "rustls-pki-types": "Intentional fixture-only TLS type boundary.",
    "flate2": "Intentional fixture-only compression response generator.",
    "cmake": "Intentional build dependency for the native bundled curl bridge.",
    "tempfile": "Intentional bounded-memory safety dependency: large response bodies spill to disk.",
}

HEAVYWEIGHT_TRANSITIVE = {
    "async-compression": "reqwest compression feature path",
    "comrak": "Markdown parser; currently pulls syntax-highlighting support",
    "criterion": "Benchmark-only; keep out of release builds",
    "icu_normalizer": "url/idna compatibility graph",
    "icu_properties": "url/idna compatibility graph",
    "hyper": "reqwest HTTP compatibility graph",
    "hyper-rustls": "reqwest TLS compatibility graph",
    "ring": "rustls cryptography",
    "syntect": "Comrak syntax-highlighting transitive cost",
    "tokio": "reqwest/HTTP compatibility runtime internals",
    "tower-http": "reqwest compression/HTTP middleware graph",
}

CRATE_NAME_OVERRIDES = {
    "percent-encoding": "percent_encoding",
    "serde-json": "serde_json",
    "rustls-pemfile": "rustls_pemfile",
    "rustls-pki-types": "rustls_pki_types",
}


@dataclass
class CommandResult:
    command: list[str]
    returncode: int
    stdout: str
    stderr: str


@dataclass
class DirectDependency:
    name: str
    package: str
    consumer: str
    kind: str
    features: list[str]
    default_features: bool
    version_req: str
    category: str
    source_files: list[str] = field(default_factory=list)


def run_command(command: list[str], root: Path, *, required: bool = False) -> CommandResult:
    env = os.environ.copy()
    env["CARGO_NET_OFFLINE"] = "true"
    completed = subprocess.run(
        command,
        cwd=root,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    result = CommandResult(command, completed.returncode, completed.stdout, completed.stderr)
    if required and completed.returncode:
        raise RuntimeError(
            f"command failed ({completed.returncode}): {' '.join(command)}\n"
            f"{completed.stderr.strip()}"
        )
    return result


def cargo_metadata(root: Path, manifest: Path) -> tuple[dict, CommandResult]:
    command = [
        "cargo",
        "metadata",
        "--offline",
        "--locked",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
        str(manifest),
    ]
    result = run_command(command, root, required=True)
    return __import__("json").loads(result.stdout), result


def cargo_tree(root: Path, args: list[str]) -> CommandResult:
    return run_command(["cargo", "tree", "--offline", "--locked", *args], root)


def package_kind(package_name: str, dep_kind: str) -> str:
    if package_name in BENCHMARK_PACKAGES:
        return "direct benchmark"
    if package_name in FIXTURE_PACKAGES:
        return "direct fixture/test"
    if package_name in FUZZ_PACKAGES:
        return "direct fuzz"
    if dep_kind == "build":
        return "direct build"
    if dep_kind == "dev":
        return "direct dev"
    return "direct runtime"


def package_classification(consumer: str, dep_kind: str, dependency: str) -> str:
    if consumer in BENCHMARK_PACKAGES:
        return "direct dev/benchmark"
    if consumer in FIXTURE_PACKAGES:
        return "direct fixture/test"
    if consumer in FUZZ_PACKAGES:
        return "direct fuzz/tooling"
    if dep_kind == "build":
        return "direct build"
    if dependency in INTENTIONAL_COMPATIBILITY:
        return "direct runtime / compatibility"
    return "direct runtime"


def crate_name(package: str) -> str:
    return CRATE_NAME_OVERRIDES.get(package, package.replace("-", "_"))


def source_evidence(package_root: Path, package: str) -> list[str]:
    needle = crate_name(package)
    patterns = [
        re.compile(rf"\b{re.escape(needle)}::"),
        re.compile(rf"\buse\s+{re.escape(needle)}\b"),
        re.compile(rf"\bextern\s+crate\s+{re.escape(needle)}\b"),
    ]
    evidence: list[str] = []
    for path in sorted(package_root.rglob("*.rs")):
        if any(part in {"target", ".git"} for part in path.parts):
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if any(pattern.search(text) for pattern in patterns):
            evidence.append(str(path.relative_to(package_root)).replace(os.sep, "/"))
    for path in sorted(package_root.glob("build.rs")):
        try:
            text = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if any(pattern.search(text) for pattern in patterns):
            evidence.append(str(path.relative_to(package_root)).replace(os.sep, "/"))
    return evidence


def collect_direct_dependencies(metadata_sets: Iterable[dict], root: Path) -> list[DirectDependency]:
    dependencies: list[DirectDependency] = []
    seen: set[tuple[str, str, str, str]] = set()
    local_package_names = {
        package["name"]
        for metadata in metadata_sets
        for package in metadata.get("packages", [])
    }
    for metadata in metadata_sets:
        for package in metadata.get("packages", []):
            package_name = package["name"]
            package_root = Path(package["manifest_path"]).parent
            for dep in package.get("dependencies", []):
                kind = dep.get("kind") or "runtime"
                dep_name = dep.get("rename") or dep["name"]
                key = (package_name, dep_name, kind, dep.get("req", "*"))
                if key in seen:
                    continue
                seen.add(key)
                # Cargo metadata --no-deps marks registry dependencies with a
                # registry source and path dependencies with no source.  Keep
                # both: the report also documents the internal workspace graph.
                dependency_name = dep["name"]
                category = package_classification(package_name, kind, dependency_name)
                dependencies.append(
                    DirectDependency(
                        name=dep_name,
                        package=dependency_name,
                        consumer=package_name,
                        kind=kind,
                        features=sorted(dep.get("features", [])),
                        default_features=bool(dep.get("uses_default_features", True)),
                        version_req=dep.get("req", "*"),
                        category=category,
                        source_files=source_evidence(package_root, dependency_name)
                        if dependency_name not in local_package_names
                        else [],
                    )
                )
    return sorted(dependencies, key=lambda item: (item.category, item.name, item.consumer))


def lock_packages(root: Path) -> list[dict]:
    with (root / "Cargo.lock").open("rb") as handle:
        data = tomllib.load(handle)
    return data.get("package", [])


def duplicate_versions(packages: list[dict]) -> dict[str, list[str]]:
    versions: dict[str, set[str]] = collections.defaultdict(set)
    for package in packages:
        versions[package["name"]].add(package["version"])
    return {name: sorted(values) for name, values in sorted(versions.items()) if len(values) > 1}


def active_workspace_packages(metadata_sets: Iterable[dict]) -> dict[str, dict]:
    result: dict[str, dict] = {}
    for metadata in metadata_sets:
        for package in metadata.get("packages", []):
            result[package["name"]] = package
    return result


def transitive_matches(packages: list[dict]) -> list[tuple[str, str, str]]:
    result = []
    for package in packages:
        name = package["name"]
        if name not in HEAVYWEIGHT_TRANSITIVE:
            continue
        source = "workspace/path" if not package.get("source") else package["source"].split("#", 1)[0]
        result.append((name, package["version"], source))
    return sorted(set(result))


def cargo_feature_evidence(root: Path, package: str) -> str:
    result = cargo_tree(root, ["--workspace", "-e", "features", "-i", package])
    if result.returncode:
        return f"unavailable (exit {result.returncode}): {result.stderr.strip()}"
    lines = result.stdout.splitlines()
    interesting = [line for line in lines if package in line or "feature" in line]
    return "\n".join(interesting[:80]) or "no feature lines returned"


def markdown_escape(value: str) -> str:
    return value.replace("|", "\\|").replace("\n", " ")


def render_report(
    root: Path,
    metadata_sets: list[dict],
    dependencies: list[DirectDependency],
    packages: list[dict],
    duplicate: dict[str, list[str]],
    duplicate_result: CommandResult,
    feature_evidence: dict[str, str],
    metadata_commands: list[str],
) -> str:
    package_map = active_workspace_packages(metadata_sets)
    lock_unique = len({(item["name"], item["version"], item.get("source")) for item in packages})
    registry_count = sum(1 for item in packages if item.get("source"))
    local_count = len(packages) - registry_count
    external_direct = [
        item
        for item in dependencies
        if item.package not in package_map
    ]
    grouped: dict[str, list[DirectDependency]] = collections.defaultdict(list)
    for item in external_direct:
        grouped[item.package].append(item)

    lines = [
        "# MDOK Dependency Audit",
        "",
        "> Generated by `scripts/audit_dependencies.py` with Cargo offline metadata/tree commands. This report is an audit baseline, not an authorization to remove a dependency.",
        "",
        f"- Repository: `{root}`",
        f"- Active metadata packages: {len(package_map)}",
        f"- Cargo.lock records: {len(packages)} ({lock_unique} unique name/version/source records; {registry_count} registry, {local_count} local/path)",
        f"- Duplicate package names with multiple locked versions: {len(duplicate)}",
        "- Network policy: Cargo commands run with `--offline` and `CARGO_NET_OFFLINE=true`.",
        "",
        "## Executive findings",
        "",
        "- The unused CLI/runtime declarations and the stale Tree-sitter parser declarations were removed in the preceding performance commits after source-import and workspace checks. This report records the resulting graph.",
        "- `reqwest` is the largest intentional runtime trade-off: it preserves curl compatibility outside the native libcurl-safe subset and enables blocking HTTP, cookies, compression, multipart, JSON, and rustls features.",
        "- `comrak` is a required Markdown parser but currently brings a substantial syntax-highlighting graph including `syntect`; verify whether that feature is needed before attempting a parser replacement.",
        "- `criterion` is benchmark-only and must remain outside shipped CLI dependencies; the manifest already scopes it to `mdok-benchmarks`.",
        "- Duplicate locked versions are limited to the names listed below. They are transitive and should be reduced only after verifying the upstream constraints in `cargo tree -d`.",
        "",
        "## Direct dependency inventory",
        "",
        "| Dependency | Classification | Consumers | Features | Source evidence / use |",
        "| --- | --- | --- | --- | --- |",
    ]
    for name in sorted(grouped):
        items = grouped[name]
        consumers = "; ".join(
            f"{item.consumer} ({item.category.replace('direct ', '')})" for item in items
        )
        features = sorted({feature for item in items for feature in item.features})
        defaults = {item.default_features for item in items}
        feature_text = ", ".join(features) if features else "default features"
        if defaults == {False}:
            feature_text += "; defaults off"
        evidence = sorted({file for item in items for file in item.source_files})
        note = USAGE_NOTES.get(name, "See source evidence and manifest consumers.")
        if name in INTENTIONAL_COMPATIBILITY:
            note = INTENTIONAL_COMPATIBILITY[name] + " " + note
        if evidence:
            evidence_text = f"{note} Files: `{', '.join(evidence)}`"
        else:
            evidence_text = note
        classifications = sorted({item.category for item in items})
        lines.append(
            "| "
            + " | ".join(
                [
                    f"`{markdown_escape(name)}`",
                    markdown_escape(", ".join(classifications)),
                    markdown_escape(consumers),
                    markdown_escape(feature_text),
                    markdown_escape(evidence_text),
                ]
            )
            + " |"
        )

    lines += [
        "",
        "### Internal workspace dependencies",
        "",
        "Local `mdok-*` crates are direct path dependencies, not external licensing or download costs. They should be profiled at the crate boundary before considering consolidation: `mdok-curl`/`mdok-curl-sys` are the native/compatibility boundary, while `mdok-markdown`, `mdok-shell`, `mdok-template`, `mdok-jmespath`, and `mdok-report` isolate parser/runtime concerns.",
        "",
        "## Direct dependency classes",
        "",
        "| Class | Meaning | Current members |",
        "| --- | --- | --- |",
        "| Direct runtime | Used by shipped CLI/runtime crates | External rows classified `direct runtime` or `direct runtime / compatibility` |",
        "| Direct dev/benchmark | Used only by benchmark harnesses | `criterion` and `mdok-benchmarks` graph |",
        "| Direct fixture/test | Local HTTPS/compression fixture support | `flate2`, `rustls`, `rustls-pemfile`, `rustls-pki-types`, `sha2` through `mdok-test-server` |",
        "| Direct build | Build-time native bridge setup | `cmake` through `mdok-curl-sys` |",
        "| Direct fuzz/tooling | Fuzz target only; separate manifest | `libfuzzer-sys` and fuzz target path dependencies |",
        "| Intentional compatibility | Preserves behavior or policy boundaries | `reqwest`, TLS/compression fixture support, `tempfile`, native `cmake` bridge |",
        "",
        "## Transitive heavyweight candidates",
        "",
        "These are investigation targets, not removal recommendations. A candidate is actionable only after a feature-minimized build, full corpus/differential tests, release smoke, and binary/RSS measurements.",
        "",
        "| Package | Locked version(s) | Why it is present | Safe next experiment |",
        "| --- | --- | --- | --- |",
    ]
    for name, version, source in transitive_matches(packages):
        reason = HEAVYWEIGHT_TRANSITIVE[name]
        if name == "comrak":
            experiment = "Inspect Comrak feature graph and test disabling highlighting-only features; preserve AST/source-position behavior."
        elif name == "criterion":
            experiment = "Confirm release package/build graph excludes the benchmark crate; do not optimize runtime around this node."
        elif name in {"reqwest", "hyper", "hyper-rustls", "tower-http", "async-compression", "ring", "icu_normalizer", "icu_properties"}:
            experiment = "Measure native-eligible and compatibility-fallback paths separately; remove one reqwest feature at a time with corpus coverage."
        else:
            experiment = "Use `cargo tree -i` and a feature-minimized build; do not change lockfile until behavior and size are measured."
        lines.append(f"| `{name}` | `{version}` ({source}) | {reason} | {experiment} |")

    lines += [
        "",
        "## Duplicate locked versions",
        "",
    ]
    if duplicate:
        lines += [
            "| Package | Locked versions | Follow-up |",
            "| --- | --- | --- |",
        ]
        for name, versions in duplicate.items():
            lines.append(
                f"| `{name}` | {', '.join(f'`{version}`' for version in versions)} | Check `cargo tree -d` parents before attempting an upgrade or feature unification. |"
            )
    else:
        lines.append("No duplicate package names were reported by the lockfile parser.")

    lines += [
        "",
        "### Raw duplicate-tree command status",
        "",
        f"`cargo tree --offline --locked --workspace --duplicates` exit: `{duplicate_result.returncode}`.",
        "",
    ]
    if duplicate_result.stdout.strip():
        lines += ["```text", duplicate_result.stdout.strip(), "```"]
    else:
        lines.append("The command returned no duplicate-tree output.")

    lines += [
        "",
        "## Feature evidence for high-cost roots",
        "",
    ]
    for package, evidence in feature_evidence.items():
        lines += [f"### `{package}`", "", "```text", evidence, "```", ""]

    lines += [
        "## Safe reduction backlog",
        "",
        "1. Build and measure a CLI-only release graph. Verify `mdok-benchmarks`, `criterion`, and fixture-server-only crates cannot enter the shipped binary package.",
        "2. Split `reqwest` compatibility features by behavior: test `blocking`, `cookies`, `brotli`, `deflate`, `gzip`, `json`, `multipart`, and `rustls-tls` independently against the strict curl differential corpus before removing any feature.",
        "3. Inspect Comrak’s enabled/default feature graph. A syntax-highlighting reduction is attractive only if Markdown AST/source positions and all PRD fences remain identical.",
        "4. Keep the custom restricted-shell parser covered by its corpus, fuzz smoke, and strict differential checks; do not reintroduce a parser dependency without measured benefit and a source-position compatibility plan.",
        "5. Investigate duplicate `getrandom`, `syn`, `winnow`, `cpufeatures`, and `windows-sys` versions through their inverse trees. Prefer upstream upgrades or feature unification over patches.",
        "6. Keep `tempfile` until the body spill threshold and low-memory guarantees are covered by RSS tests; replacing it with an ad-hoc file path would weaken safety.",
        "7. Re-run this audit after every manifest, feature, or lockfile change and attach the generated report to the performance review.",
        "",
        "## Repeatable commands and exit behavior",
        "",
        "```sh",
        "# Offline, locked audit; writes docs/DEPENDENCY_AUDIT.md",
        "python3 scripts/audit_dependencies.py",
        "",
        "# Write to another path without changing manifests or Cargo.lock",
        "python3 scripts/audit_dependencies.py --output /tmp/mdok-dependency-audit.md",
        "",
        "# Strict mode: exit 3 when a policy blocker or unavailable Cargo evidence is present",
        "python3 scripts/audit_dependencies.py --strict",
        "```",
        "",
        "- Exit `0`: audit completed and generated the report.",
        "- Exit `2`: required local input or Cargo metadata/tree command failed; inspect stderr and restore the offline cache/lockfile state.",
        "- Exit `3`: `--strict` detected an audit blocker such as unavailable evidence or a prohibited license marker. Normal transitive duplicates and investigation candidates do not fail default mode.",
        "- The script never runs `cargo update`, edits manifests, edits `Cargo.lock`, contacts a registry, or removes a dependency.",
        "",
        "## Source policy reference",
        "",
        "The audit follows `mdok-prd/docs/18-dependencies-and-licensing.md`: prefer mature standards parsers, minimize unsafe/build-time execution, reject GPL dependencies in the shipped binary, and scrutinize parsing, TLS, serialization, and FFI dependencies.",
        "",
        "## Audit command record",
        "",
    ]
    for command in metadata_commands:
        lines.append(f"- `{command}`")
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=SCRIPT_ROOT, help="workspace root (default: repository root)")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT, help="Markdown report path")
    parser.add_argument("--strict", action="store_true", help="fail with exit 3 when evidence/tooling blockers are found")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    output = args.output if args.output.is_absolute() else root / args.output
    output = output.resolve()
    metadata_commands: list[str] = []
    warnings: list[str] = []
    try:
        manifests = [root / "Cargo.toml"]
        fuzz_manifest = root / "fuzz" / "Cargo.toml"
        if fuzz_manifest.exists():
            manifests.append(fuzz_manifest)
        metadata_sets: list[dict] = []
        for manifest in manifests:
            command = [
                "cargo",
                "metadata",
                "--offline",
                "--locked",
                "--format-version",
                "1",
                "--no-deps",
                "--manifest-path",
                str(manifest),
            ]
            metadata_commands.append(" ".join(command))
            metadata, _ = cargo_metadata(root, manifest)
            metadata_sets.append(metadata)

        dependencies = collect_direct_dependencies(metadata_sets, root)
        packages = lock_packages(root)
        duplicate = duplicate_versions(packages)
        duplicate_result = cargo_tree(root, ["--workspace", "--duplicates"])
        if duplicate_result.returncode:
            warnings.append("cargo tree duplicate analysis failed")

        feature_evidence = {
            package: cargo_feature_evidence(root, package)
            for package in ("reqwest", "comrak", "criterion")
        }
        for package, evidence in feature_evidence.items():
            if evidence.startswith("unavailable"):
                warnings.append(f"feature evidence unavailable for {package}")

        report = render_report(
            root,
            metadata_sets,
            dependencies,
            packages,
            duplicate,
            duplicate_result,
            feature_evidence,
            metadata_commands,
        )
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(report, encoding="utf-8")
    except (OSError, RuntimeError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"audit_dependencies.py: error: {error}", file=sys.stderr)
        return 2

    if warnings:
        print("audit_dependencies.py: warnings:", file=sys.stderr)
        for warning in warnings:
            print(f"- {warning}", file=sys.stderr)
        if args.strict:
            return 3
    print(f"wrote {output}")
    print(f"locked duplicate package names: {len(duplicate)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
