#!/usr/bin/env python3
"""Compare the bundled curl parser with MDOK's planning contract.

The harness deliberately exercises curl with ``--help`` after each option so
curl parses the complete argv without opening a network connection.  MDOK is
run in ``plan --json`` mode against generated one-step Markdown documents.
The generated cases cover every canonical option in curl's help table, every
curl short alias, and every explicit option/alias row in the MDOK policy.

This is a parser/planner test only.  It does not execute transfers and does
not link to or modify MDOK's native runtime.
"""

from __future__ import annotations

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "specs/curl-option-policy.csv"
CURL_HELP_PATH = ROOT / "vendor/curl/src/tool_listhelp.c"
CURL_VERSION_PATH = ROOT / "vendor/curl.version"
DEFAULT_CURL_BUILD = ROOT / "target/differential/curl-build"
DEFAULT_MDOK = ROOT / "target/debug/mdok"
DIFFERENTIAL_URL = "http://127.0.0.1:1/mdok-differential"

ACCEPTING_CLASSES = {"transfer", "compatibility-noop", "virtualized"}
REJECTING_CLASSES = {"unsupported", "policy-gated"}
STATIC_POLICY_GATED_OPTIONS = {
    "--cacert",
    "--cert",
    "--config",
    "--connect-to",
    "--data-binary",
    "--form",
    "--insecure",
    "--key",
    "--proxy",
    "--resolve",
    "--upload-file",
}
POLICY_ERROR_CODES = {
    "MDOK-E300",
    "MDOK-E301",
    "MDOK-E302",
    "MDOK-E303",
    "MDOK-E304",
    "MDOK-E602",
    "MDOK-E604",
}
REPEATABLE_OPTIONS = {
    "--connect-to",
    "--cookie",
    "--data",
    "--data-ascii",
    "--data-binary",
    "--data-raw",
    "--data-urlencode",
    "--form",
    "--form-string",
    "--header",
    "--oauth2-bearer",
    "--proxy-header",
    "--referer",
    "--request",
    "--resolve",
    "--url-query",
    "--user",
    "--user-agent",
}


@dataclass(frozen=True)
class OptionSpec:
    long: str
    aliases: tuple[str, ...]
    takes_value: bool
    optional_value: bool


@dataclass(frozen=True)
class Case:
    case_id: str
    option: str
    canonical: str
    classification: str
    area: str
    kind: str
    explicit_policy: bool
    takes_value: bool
    optional_value: bool
    sample: str | None
    command: tuple[str, ...]
    document_name: str
    plan_contains: tuple[str, ...] = ()


def run_command(
    args: list[str],
    *,
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
    timeout: float = 30.0,
) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        args,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )


def load_policy() -> dict[str, dict[str, str]]:
    with POLICY_PATH.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    if not rows or set(rows[0]) != {"option", "classification", "area"}:
        raise RuntimeError(f"invalid policy header: {POLICY_PATH}")
    policy: dict[str, dict[str, str]] = {}
    for row in rows:
        option = row["option"]
        if option in policy:
            raise RuntimeError(f"duplicate policy row: {option}")
        policy[option] = row
    return policy


def load_curl_options() -> list[OptionSpec]:
    text = CURL_HELP_PATH.read_text(encoding="utf-8")
    options: list[OptionSpec] = []
    seen: set[str] = set()
    entry_pattern = re.compile(r'^\s*\{\s*"([^"]+)"\s*,')
    long_pattern = re.compile(r"--[A-Za-z0-9][A-Za-z0-9.-]*")
    for line in text.splitlines():
        match = entry_pattern.match(line)
        if not match:
            continue
        help_line = match.group(1)
        long_match = long_pattern.search(help_line)
        if not long_match:
            continue
        long = long_match.group(0)
        if long in seen:
            raise RuntimeError(f"duplicate curl help option: {long}")
        seen.add(long)
        left = help_line.split(",", 1)[0].strip()
        aliases = (left,) if left.startswith("-") and not left.startswith("--") else ()
        takes_value = bool(
            re.search(rf"{re.escape(long)}\s+<[^>]+>", help_line)
        )
        options.append(
            OptionSpec(
                long=long,
                aliases=aliases,
                takes_value=takes_value,
                optional_value=long == "--help",
            )
        )
    if not options:
        raise RuntimeError(f"no curl options found in {CURL_HELP_PATH}")
    return options


def build_curl(cmake: str, build_dir: Path, jobs: int, rebuild: bool) -> Path:
    build_dir.mkdir(parents=True, exist_ok=True)
    binary_candidates = (
        build_dir / "src/curl",
        build_dir / "src/curl.exe",
        build_dir / "src/Release/curl.exe",
        build_dir / "src/Debug/curl.exe",
        build_dir / "curl",
        build_dir / "curl.exe",
    )
    if rebuild or not any(path.is_file() for path in binary_candidates):
        configure = [
            cmake,
            "-S",
            str(ROOT / "vendor/curl"),
            "-B",
            str(build_dir),
            "-DBUILD_CURL_EXE=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DBUILD_STATIC_LIBS=ON",
            "-DBUILD_TESTING=OFF",
            "-DCURL_DISABLE_INSTALL=ON",
            "-DCURL_DISABLE_LDAP=ON",
            "-DCURL_DISABLE_LDAPS=ON",
            "-DCURL_USE_OPENSSL=OFF",
            "-DCURL_ZLIB=OFF",
            "-DCURL_BROTLI=OFF",
            "-DCURL_ZSTD=OFF",
            "-DUSE_NGHTTP2=OFF",
            "-DUSE_NGTCP2=OFF",
            "-DUSE_QUICHE=OFF",
            "-DCURL_USE_LIBPSL=OFF",
            "-DCURL_USE_LIBSSH2=OFF",
            "-DCURL_USE_LIBSSH=OFF",
            "-DCURL_USE_GSSAPI=OFF",
            "-DCURL_USE_GSASL=OFF",
        ]
        configure_result = run_command(configure, timeout=180)
        if configure_result.returncode:
            raise RuntimeError(
                "bundled curl CMake configure failed:\n"
                + summarize_output(configure_result.stderr)
            )
        build = [cmake, "--build", str(build_dir), "--target", "curl"]
        if jobs > 0:
            build.extend(["--parallel", str(jobs)])
        build_result = run_command(build, timeout=300)
        if build_result.returncode:
            raise RuntimeError(
                "bundled curl build failed:\n" + summarize_output(build_result.stderr)
            )
    for path in binary_candidates:
        if path.is_file():
            return path
    raise RuntimeError(f"bundled curl executable not found under {build_dir}")


def find_executable(path: Path | None, default: Path, label: str) -> Path:
    if path is not None:
        candidate = path.expanduser().resolve()
    else:
        candidate = default
    if not candidate.is_file():
        raise RuntimeError(
            f"{label} executable not found: {candidate}; pass --{label} or build it first"
        )
    return candidate


def decode_output(data: bytes) -> str:
    return data.decode("utf-8", errors="replace").replace("\r\n", "\n")


def summarize_output(data: bytes, limit: int = 600) -> str:
    text = decode_output(data)
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    summary = " | ".join(lines[:3])
    return summary[:limit]


def curl_error_kind(stderr: str, returncode: int) -> str:
    lower = stderr.lower()
    if returncode == 0:
        return "accepted"
    if "is unknown" in lower or "unknown option" in lower:
        return "unknown-option"
    if "requires parameter" in lower or "requires a parameter" in lower:
        return "missing-parameter"
    if (
        "doesn't support" in lower
        or "does not support" in lower
        or "not built in" in lower
        or "unsupported by libcurl" in lower
    ):
        return "feature-unavailable"
    if "expected a proper" in lower or "invalid" in lower:
        return "invalid-parameter"
    if "no URL specified".lower() in lower:
        return "missing-url"
    return "other-error"


def normalize_text(text: str, root: Path, temporary: Path) -> str:
    return (
        text.replace(str(root), "<repo>")
        .replace(str(temporary), "<temporary>")
        .replace("\\", "/")
    )


def sample_for(option: str, fixture: Path, outside_file: Path, config_file: Path, artifact: Path) -> str:
    if option in {"--cacert", "--cert", "--key", "--data-binary", "--upload-file"}:
        return f"@{outside_file}" if option == "--data-binary" else str(outside_file)
    if option in {"--cookie", "--form"}:
        return "session=hello" if option == "--cookie" else f"field=@{outside_file}"
    if option == "--config":
        return str(config_file)
    if option == "--cookie-jar":
        return str(artifact)
    if option in {"--data", "--data-ascii", "--data-raw"}:
        return "message=hello"
    if option == "--data-urlencode":
        return "message=hello world"
    if option == "--json":
        return '{"message":"hello"}'
    if option in {"--header", "--proxy-header"}:
        return "X-MDOK-Differential: yes"
    if option in {"--request", "--request-target"}:
        return "GET" if option == "--request" else "/mdok-differential"
    if option in {"--user", "--proxy-user"}:
        return "mdok:secret"
    if option == "--oauth2-bearer":
        return "differential-token"
    if option in {"--user-agent", "--referer"}:
        return "mdok-differential" if option == "--user-agent" else "https://example.com/"
    if option in {"--connect-timeout", "--max-time", "--retry-delay", "--retry-max-time"}:
        return "1"
    if option in {"--max-redirs", "--retry", "--parallel-max", "--parallel-max-host"}:
        return "1"
    if option in {
        "--continue-at",
        "--create-file-mode",
        "--expect100-timeout",
        "--happy-eyeballs-timeout-ms",
        "--keepalive-cnt",
        "--keepalive-time",
        "--limit-rate",
        "--max-filesize",
        "--speed-limit",
        "--speed-time",
        "--tftp-blksize",
        "--vlan-priority",
    }:
        return "1"
    if option in {"--range"}:
        return "0-10"
    if option in {"--resolve"}:
        return "example.com:80:127.0.0.1"
    if option in {"--connect-to"}:
        return "example.com:80:example.org:80"
    if option in {"--proxy", "--preproxy"}:
        return "http://127.0.0.1:1"
    if option in {"--proxy1.0", "--socks4", "--socks4a", "--socks5", "--socks5-hostname"}:
        return "127.0.0.1:1"
    if option in {"--doh-url", "--ipfs-gateway"}:
        return "https://example.com/"
    if option in {"--url", "--referer"}:
        return DIFFERENTIAL_URL
    if option in {"--url-query"}:
        return "message=hello"
    if option in {"--proto", "--proto-default", "--proto-redir", "--noproxy"}:
        return "https" if option != "--noproxy" else "example.com"
    if option in {"--cert-type", "--key-type", "--proxy-cert-type"}:
        return "PEM"
    if option in {"--tls-max"}:
        return "1.3"
    if option in {"--aws-sigv4"}:
        return "aws:amz:us-east-1:execute-api"
    if option in {"--variable"}:
        return "name=value"
    if option in {"--help"}:
        return "all"
    if option in {"--rate"}:
        return "1/s"
    if option in {"--telnet-option"}:
        return "option=value"
    if option in {"--time-cond"}:
        return "now"
    if option in {"--pinnedpubkey"}:
        return "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    if option in {"--proxy-tlsauthtype", "--tlsauthtype"}:
        return "SRP"
    if option in {"--proxy-tlspassword", "--tlspassword", "--pass"}:
        return "password"
    if option in {"--proxy-tlsuser", "--tlsuser", "--proxy-service-name", "--service-name"}:
        return "mdok"
    if option in {"--proxy-tls13-ciphers", "--tls13-ciphers", "--ciphers", "--curves", "--sigalgs"}:
        return "DEFAULT"
    if option in {"--rate", "--limit-rate"}:
        return "1K"
    if option in {"--ftp-method"}:
        return "multicwd"
    if option in {"--ftp-port"}:
        return "127.0.0.1:1"
    if option in {"--ftp-ssl-ccc-mode"}:
        return "passive"
    if option in {"--proxy-pinnedpubkey"}:
        return "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    if option in {"--trace-config"}:
        return "ids"
    if option in {"--url-query"}:
        return "message=hello"
    if option in {"--abstract-unix-socket", "--unix-socket"}:
        return str(outside_file)
    if option in {
        "--alt-svc",
        "--capath",
        "--crlfile",
        "--dump-header",
        "--egd-file",
        "--etag-compare",
        "--etag-save",
        "--hsts",
        "--knownhosts",
        "--libcurl",
        "--netrc-file",
        "--output",
        "--output-dir",
        "--proxy-cacert",
        "--proxy-capath",
        "--proxy-cert",
        "--proxy-crlfile",
        "--proxy-key",
        "--random-file",
        "--ssl-sessions",
        "--stderr",
        "--trace",
        "--trace-ascii",
        "--pubkey",
    }:
        return str(outside_file)
    if option in {"--mail-auth", "--mail-from", "--mail-rcpt"}:
        return "mdok@example.com"
    if option in {"--ftp-account", "--ftp-alternative-to-user", "--quote"}:
        return "mdok"
    if option in {"--delegation", "--krb"}:
        return "none"
    if option in {"--dns-interface", "--interface"}:
        return "lo0"
    if option in {"--dns-ipv4-addr", "--dns-ipv6-addr", "--haproxy-clientip"}:
        return "127.0.0.1"
    if option == "--hostpubmd5":
        return "0123456789abcdef0123456789abcdef"
    if option == "--ip-tos":
        return "1"
    if option == "--dns-servers":
        return "127.0.0.1"
    if option in {"--ech", "--ip-tos", "--login-options", "--sasl-authzid", "--upload-flags"}:
        return "value"
    if option in {"--engine", "--hostpubmd5", "--hostpubsha256", "--mail-auth"}:
        return "value"
    if option in {"--form-string"}:
        return "field=value"
    if option in {"--ftp-ssl-ccc-mode"}:
        return "passive"
    if option in {"--local-port"}:
        return "1"
    if option in {"--proxy-pass"}:
        return "password"
    if option in {"--proxy-key-type"}:
        return "PEM"
    if option in {"--proxy-service-name"}:
        return "mdok"
    if option in {"--speed-limit", "--speed-time"}:
        return "1"
    if option in {"--stderr"}:
        return str(outside_file)
    if option in {"--write-out"}:
        return "%{http_code}"
    if option in {"--vlan-priority"}:
        return "1"
    return "value"


def repeat_sample_for(option: str, first: str, outside_file: Path) -> str:
    if option in {"--header", "--proxy-header"}:
        return "X-MDOK-Differential: repeated"
    if option in {"--data", "--data-ascii", "--data-raw", "--data-urlencode"}:
        return "message=repeated"
    if option == "--data-binary":
        return f"@{outside_file}"
    if option == "--cookie":
        return "session=repeated"
    if option in {"--form", "--form-string"}:
        return f"field=repeated" if option == "--form-string" else f"field=@{outside_file}"
    if option == "--connect-to":
        return "example.com:80:example.net:80"
    if option == "--resolve":
        return "example.com:80:127.0.0.1"
    if option == "--request":
        return "POST"
    if option in {"--oauth2-bearer", "--user-agent", "--referer"}:
        return "mdok-differential-repeated"
    if option == "--user":
        return "repeated:secret"
    if option == "--url-query":
        return "repeated=yes"
    return first


def shell_word(value: str) -> str:
    if re.fullmatch(r"[A-Za-z0-9_./:+%=@?,\-]+", value):
        return value
    return "'" + value.replace("'", "'\\''") + "'"


def command_text(command: Iterable[str]) -> str:
    return " ".join(shell_word(word) for word in command)


def case_slug(value: str) -> str:
    slug = re.sub(r"[^A-Za-z0-9]+", "_", value.lstrip("-"))
    return slug.strip("_") or "option"


def make_cases(
    options: list[OptionSpec],
    policy: dict[str, dict[str, str]],
    fixture: Path,
    outside_file: Path,
    config_file: Path,
    artifact: Path,
    limit: int | None,
) -> list[Case]:
    by_long = {option.long: option for option in options}
    missing_long = sorted(set(by_long) - set(policy))
    stale_long = sorted(option for option in policy if option.startswith("--") and option not in by_long)
    if missing_long:
        raise RuntimeError("curl options missing from policy: " + ", ".join(missing_long))
    if stale_long:
        raise RuntimeError("policy options missing from curl help: " + ", ".join(stale_long))

    aliases: dict[str, str] = {}
    for option in options:
        for alias in option.aliases:
            if alias in aliases:
                raise RuntimeError(f"short alias maps to multiple options: {alias}")
            aliases[alias] = option.long

    for option in policy:
        if option.startswith("-") and not option.startswith("--") and option not in aliases:
            raise RuntimeError(f"policy short alias missing from curl help: {option}")
        if option.startswith("-") and not option.startswith("--"):
            canonical = aliases[option]
            if policy[option]["classification"] != policy[canonical]["classification"]:
                raise RuntimeError(f"policy alias classification differs from {canonical}: {option}")

    selected = options if limit is None else options[:limit]
    cases: list[Case] = []
    index = 0

    def add_case(
        option: str,
        canonical: str,
        kind: str,
        explicit_policy: bool,
        spec: OptionSpec,
        sample: str | None,
        command: tuple[str, ...],
        plan_contains: tuple[str, ...] = (),
    ) -> None:
        nonlocal index
        row = policy[option] if explicit_policy else policy[canonical]
        case_id = f"diff_{index:04d}_{case_slug(kind)}_{case_slug(canonical)}"
        if kind in {"alias", "repeat-alias"}:
            case_id += f"_{case_slug(option)}"
        document_name = f"{case_id}.md"
        cases.append(
            Case(
                case_id=case_id,
                option=option,
                canonical=canonical,
                classification=row["classification"],
                area=row["area"],
                kind=kind,
                explicit_policy=explicit_policy,
                takes_value=spec.takes_value,
                optional_value=spec.optional_value,
                sample=sample,
                command=command,
                document_name=document_name,
                plan_contains=plan_contains,
            )
        )
        index += 1

    for spec in selected:
        sample = sample_for(spec.long, fixture, outside_file, config_file, artifact) if spec.takes_value else None
        if spec.long == "--next":
            # curl requires a completed URL before --next; this also
            # exercises MDOK's one-transfer rejection for the same argv.
            valid_command = ["curl", DIFFERENTIAL_URL, spec.long, DIFFERENTIAL_URL]
        else:
            valid_command = ["curl", spec.long]
        if sample is not None:
            valid_command.append(sample)
        if spec.long != "--next":
            valid_command.append(DIFFERENTIAL_URL)
        valid_command.append("--help")
        add_case(
            spec.long,
            spec.long,
            "canonical",
            True,
            spec,
            sample,
            tuple(valid_command),
        )
        for alias in spec.aliases:
            alias_sample = sample
            if spec.long == "--next":
                alias_command = ["curl", DIFFERENTIAL_URL, alias, DIFFERENTIAL_URL]
            else:
                alias_command = ["curl", alias]
            if alias_sample is not None:
                alias_command.append(alias_sample)
            if spec.long != "--next":
                alias_command.append(DIFFERENTIAL_URL)
            alias_command.append("--help")
            add_case(
                alias,
                spec.long,
                "alias",
                alias in policy,
                spec,
                alias_sample,
                tuple(alias_command),
            )

    for spec in selected:
        if spec.long not in REPEATABLE_OPTIONS or not spec.takes_value:
            continue
        first = sample_for(spec.long, fixture, outside_file, config_file, artifact)
        second = repeat_sample_for(spec.long, first, outside_file)
        command = (
            "curl",
            spec.long,
            first,
            spec.long,
            second,
            DIFFERENTIAL_URL,
            "--help",
        )
        add_case(spec.long, spec.long, "repeat", True, spec, second, command)
        for alias in spec.aliases:
            alias_command = (
                "curl",
                alias,
                first,
                alias,
                second,
                DIFFERENTIAL_URL,
                "--help",
            )
            add_case(
                alias,
                spec.long,
                "repeat-alias",
                alias in policy,
                spec,
                second,
                alias_command,
            )

    for spec in selected:
        if (
            spec.takes_value
            or spec.long.startswith("--no-")
            or spec.long in {"--help", "--manual", "--version"}
        ):
            continue
        negated = "--no-" + spec.long.removeprefix("--")
        command = ("curl", negated, DIFFERENTIAL_URL, "--help")
        add_case(negated, spec.long, "negated", False, spec, None, command)

    transfer_characteristics = (
        (
            "method_body",
            ("--request", "POST", "--header", "X-MDOK-Differential: method", "--data", "message=hello"),
        ),
        ("get_query", ("--get", "--data", "query=hello")),
        (
            "redirect_timeout",
            ("--location", "--max-redirs", "2", "--max-time", "1"),
        ),
        ("auth_range", ("--user", "mdok:secret", "--range", "0-10")),
        ("compression_http", ("--compressed", "--http1.1")),
    )
    for name, arguments in transfer_characteristics:
        command = ("curl", *arguments, DIFFERENTIAL_URL, "--help")
        case_id = f"diff_{index:04d}_transfer_{name}"
        cases.append(
            Case(
                case_id=case_id,
                option=name,
                canonical=name,
                classification="transfer",
                area="characteristics",
                kind="transfer-characteristic",
                explicit_policy=False,
                takes_value=False,
                optional_value=False,
                sample=None,
                command=command,
                document_name=f"{case_id}.md",
                plan_contains=arguments,
            )
        )
        index += 1

    for spec in selected:
        if not spec.takes_value or spec.optional_value:
            continue
        for option, kind, explicit in [(spec.long, "missing", True), *[(alias, "missing-alias", alias in policy) for alias in spec.aliases]]:
            command = ("curl", option)
            add_case(option, spec.long, kind, explicit, spec, None, command)

    unknown_spec = selected[0]
    unknown_row = policy[unknown_spec.long]
    unknown_id = f"diff_{index:04d}_unknown_option"
    cases.append(
        Case(
            case_id=unknown_id,
            option="--mdok-differential-unknown",
            canonical="--mdok-differential-unknown",
            classification="unknown",
            area="harness",
            kind="unknown",
            explicit_policy=False,
            takes_value=False,
            optional_value=False,
            sample=None,
            command=("curl", "--mdok-differential-unknown", DIFFERENTIAL_URL, "--help"),
            document_name=f"{unknown_id}.md",
        )
    )
    return cases


def write_documents(cases: list[Case], directory: Path) -> None:
    for case in cases:
        mdok_command = case.command
        if mdok_command and mdok_command[-1] == "--help":
            # --help is the curl-only parser terminator.  It must not become
            # part of the MDOK command, where it is an unsupported option.
            mdok_command = mdok_command[:-1]
        body = command_text(mdok_command)
        document = (
            f"# Differential case {case.case_id}\n\n"
            f"<!-- generated by scripts/run_curl_differential.py -->\n\n"
            f"```curl mdok name={case.case_id}\n{body}\n```\n"
        )
        (directory / case.document_name).write_text(document, encoding="utf-8")


def run_curl_case(
    curl: Path,
    case: Case,
    temporary: Path,
    curl_home: Path,
) -> dict[str, Any]:
    argv = list(case.command)
    if case.kind in {"canonical", "alias"}:
        argv = ["-q", *argv[1:]]
    else:
        argv = ["-q", *argv[1:]]
    environment = os.environ.copy()
    environment["CURL_HOME"] = str(curl_home)
    try:
        result = run_command([str(curl), *argv], env=environment, timeout=20)
    except subprocess.TimeoutExpired:
        return {
            "accepted": False,
            "returncode": None,
            "error_kind": "timeout",
            "stderr": "curl invocation timed out",
            "stdout_bytes": 0,
            "stderr_bytes": 0,
        }
    stderr = normalize_text(decode_output(result.stderr), ROOT, temporary)
    return {
        "accepted": result.returncode == 0,
        "returncode": result.returncode,
        "error_kind": curl_error_kind(stderr, result.returncode),
        "stderr": summarize_output(result.stderr),
        "stdout_bytes": len(result.stdout),
        "stderr_bytes": len(result.stderr),
    }


def collect_diagnostic_codes(value: Any) -> list[str]:
    found: list[str] = []
    if isinstance(value, dict):
        code = value.get("code")
        if isinstance(code, str) and code.startswith("MDOK-"):
            found.append(code)
        for child in value.values():
            found.extend(collect_diagnostic_codes(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(collect_diagnostic_codes(child))
    return sorted(set(found))


def run_mdok_batch(mdok: Path, config: Path, directory: Path) -> dict[str, dict[str, Any]]:
    args = [
        str(mdok),
        "--config",
        str(config),
        "--json",
        "plan",
        str(directory),
    ]
    try:
        result = run_command(args, timeout=180)
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("mdok plan batch timed out") from error
    try:
        report = json.loads(decode_output(result.stdout))
    except json.JSONDecodeError as error:
        raise RuntimeError(
            "mdok did not emit JSON:\n" + summarize_output(result.stdout + result.stderr)
        ) from error
    documents = report.get("documents")
    if not isinstance(documents, list):
        raise RuntimeError("mdok JSON report has no documents array")
    outcomes: dict[str, dict[str, Any]] = {}
    for document in documents:
        if not isinstance(document, dict):
            continue
        raw_path = str(document.get("path", ""))
        name = Path(raw_path).name
        steps = document.get("steps", [])
        planned_command = []
        if isinstance(steps, list) and steps and isinstance(steps[0], dict):
            command = steps[0].get("command", [])
            if isinstance(command, list):
                planned_command = [str(token) for token in command]
        outcomes[name] = {
            "accepted": document.get("status") == "planned"
            and not collect_diagnostic_codes(document),
            "status": document.get("status"),
            "diagnostic_codes": collect_diagnostic_codes(document),
            "planned_command": planned_command,
            "returncode": result.returncode,
        }
    return outcomes


def expected_acceptance(case: Case) -> bool | None:
    if case.kind in {"missing", "missing-alias", "unknown"}:
        return False
    if case.kind == "negated":
        return None
    if case.classification == "feature-gated":
        return None
    if case.classification in ACCEPTING_CLASSES:
        return True
    if case.classification == "policy-gated":
        # Cookie strings are in-memory transfer data; the file form is
        # exercised by the curl parser but MDOK defers its cookie policy to
        # execution.  The remaining policy probes use a denied configuration
        # or an out-of-root file and must fail during planning.
        return case.canonical not in STATIC_POLICY_GATED_OPTIONS
    if case.classification in REJECTING_CLASSES:
        return False
    raise RuntimeError(f"unknown policy classification: {case.classification}")


def compare_case(case: Case, curl_result: dict[str, Any], mdok_result: dict[str, Any]) -> tuple[bool, list[str]]:
    reasons: list[str] = []
    curl_accepted = bool(curl_result["accepted"])
    mdok_accepted = bool(mdok_result["accepted"])
    expected = expected_acceptance(case)
    if case.kind in {"missing", "missing-alias"}:
        if curl_accepted:
            reasons.append("curl accepted an option without its required argument")
        if mdok_accepted:
            reasons.append("MDOK planned an option without its required argument")
    elif case.kind == "unknown":
        if curl_result["error_kind"] != "unknown-option":
            reasons.append(f"curl error kind was {curl_result['error_kind']}, expected unknown-option")
        if mdok_accepted or "MDOK-E300" not in mdok_result["diagnostic_codes"]:
            reasons.append("MDOK did not reject the unknown option as MDOK-E300")
    elif case.kind == "negated":
        if curl_accepted and case.classification in ACCEPTING_CLASSES and not mdok_accepted:
            reasons.append("curl accepted the negated form but MDOK did not plan it")
        if mdok_accepted and (not curl_accepted or case.classification in REJECTING_CLASSES):
            reasons.append("MDOK planned a negated form that curl rejected or policy denies")
    elif case.kind == "transfer-characteristic":
        if not curl_accepted and curl_result["error_kind"] != "feature-unavailable":
            reasons.append(f"bundled curl rejected transfer characteristic ({curl_result['error_kind']})")
        if curl_result["error_kind"] != "feature-unavailable" and not mdok_accepted:
            reasons.append("MDOK did not plan the transfer characteristic")
        planned = mdok_result.get("planned_command", [])
        position = 0
        for token in case.plan_contains:
            try:
                position = planned.index(token, position) + 1
            except ValueError:
                reasons.append(f"normalized MDOK plan omitted transfer token {token!r}")
                break
    else:
        if (
            not curl_accepted
            and case.classification not in REJECTING_CLASSES
            and case.classification != "feature-gated"
            and curl_result["error_kind"] != "feature-unavailable"
        ):
            reasons.append(f"bundled curl rejected a classified option ({curl_result['error_kind']})")
        if expected is not None and mdok_accepted != expected:
            expectation = "accept" if expected else "reject"
            reasons.append(f"MDOK planned={mdok_accepted}, expected {expectation} for {case.classification}")
        if case.classification in REJECTING_CLASSES and expected is False and mdok_accepted:
            reasons.append("policy classification was accepted without a rejection")
        if case.classification == "policy-gated" and not mdok_accepted:
            unexpected_codes = set(mdok_result["diagnostic_codes"]) - POLICY_ERROR_CODES
            if unexpected_codes:
                reasons.append("policy-gated rejection used unexpected diagnostics: " + ",".join(sorted(unexpected_codes)))
    return not reasons, reasons


def compare_aliases(cases: list[Case], results: list[dict[str, Any]]) -> None:
    canonical_results = {
        case.canonical: result
        for case, result in zip(cases, results)
        if case.kind == "canonical"
    }
    repeat_results = {
        case.canonical: result
        for case, result in zip(cases, results)
        if case.kind == "repeat"
    }
    for case, result in zip(cases, results):
        if case.kind not in {"alias", "missing-alias", "repeat-alias"}:
            continue
        canonical = (
            repeat_results.get(case.canonical)
            if case.kind == "repeat-alias"
            else canonical_results.get(case.canonical)
        )
        if canonical is None:
            result["ok"] = False
            result["reasons"].append("alias has no canonical comparison case")
            continue
        if case.kind in {"alias", "repeat-alias"}:
            if result["curl"]["accepted"] != canonical["curl"]["accepted"]:
                result["ok"] = False
                result["reasons"].append("curl alias acceptance differs from canonical option")
            if result["mdok"]["accepted"] != canonical["mdok"]["accepted"]:
                result["ok"] = False
                result["reasons"].append("MDOK alias planning differs from canonical option")
        else:
            if result["curl"]["accepted"]:
                result["ok"] = False
                result["reasons"].append("curl alias accepted missing argument")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--curl", type=Path, help="use an existing bundled curl executable")
    parser.add_argument("--mdok", type=Path, default=DEFAULT_MDOK, help="mdok executable")
    parser.add_argument("--config", type=Path, default=ROOT / "mdok.toml", help="MDOK config")
    parser.add_argument("--cmake", default="cmake", help="CMake executable used to build curl")
    parser.add_argument("--curl-build-dir", type=Path, default=DEFAULT_CURL_BUILD)
    parser.add_argument("--curl-build-jobs", type=int, default=2)
    parser.add_argument("--rebuild-curl", action="store_true")
    parser.add_argument("--limit", type=int, help="limit canonical options for a quick smoke run")
    parser.add_argument("--report", type=Path, help="write the JSON report to this path")
    parser.add_argument("--keep-workdir", action="store_true", help="retain generated Markdown cases")
    parser.add_argument("--no-strict", action="store_true", help="report mismatches but exit successfully")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.limit is not None and args.limit <= 0:
        raise RuntimeError("--limit must be positive")
    if not args.config.is_file():
        raise RuntimeError(f"MDOK config not found: {args.config}")
    mdok = find_executable(args.mdok, DEFAULT_MDOK, "mdok")
    curl = build_curl(args.cmake, args.curl_build_dir, args.curl_build_jobs, args.rebuild_curl) if args.curl is None else find_executable(args.curl, args.curl_build_dir / "src/curl", "curl")

    expected_curl_version = CURL_VERSION_PATH.read_text(encoding="utf-8").strip()
    version_result = run_command([str(curl), "--version"], timeout=20)
    version_text = decode_output(version_result.stdout)
    if version_result.returncode or expected_curl_version not in version_text.splitlines()[0]:
        raise RuntimeError(
            f"curl executable does not report vendored version {expected_curl_version}: "
            + summarize_output(version_result.stdout + version_result.stderr)
        )

    policy = load_policy()
    options = load_curl_options()
    all_aliases = sum(len(option.aliases) for option in options)
    explicit_aliases = sum(
        1 for option in policy if option.startswith("-") and not option.startswith("--")
    )

    temporary_context = tempfile.TemporaryDirectory(prefix="mdok-curl-differential-")
    temporary = Path(temporary_context.name)
    try:
        curl_home = temporary / "curl-home"
        curl_home.mkdir()
        outside_file = temporary / "outside-policy-input.txt"
        outside_file.write_text("differential fixture\n", encoding="utf-8")
        config_file = temporary / "empty-curlrc"
        config_file.write_text("", encoding="utf-8")
        artifact = temporary / "cookie-artifact.txt"
        cases = make_cases(
            options,
            policy,
            ROOT / "tests/fixtures/files/hello.txt",
            outside_file,
            config_file,
            artifact,
            args.limit,
        )
        case_dir = temporary / "cases"
        case_dir.mkdir()
        write_documents(cases, case_dir)
        mdok_outcomes = run_mdok_batch(mdok, args.config.resolve(), case_dir)
        results: list[dict[str, Any]] = []
        for case in cases:
            curl_result = run_curl_case(curl, case, temporary, curl_home)
            mdok_result = mdok_outcomes.get(
                case.document_name,
                {
                    "accepted": False,
                    "status": "missing",
                    "diagnostic_codes": [],
                    "planned_command": [],
                    "returncode": None,
                },
            )
            ok, reasons = compare_case(case, curl_result, mdok_result)
            results.append(
                {
                    "case_id": case.case_id,
                    "kind": case.kind,
                    "option": case.option,
                    "canonical": case.canonical,
                    "classification": case.classification,
                    "area": case.area,
                    "explicit_policy": case.explicit_policy,
                    "sample": case.sample,
                    "command": list(case.command),
                    "curl": curl_result,
                    "mdok": mdok_result,
                    "plan_contains": list(case.plan_contains),
                    "ok": ok,
                    "reasons": reasons,
                }
            )
        compare_aliases(cases, results)
        failures = [result for result in results if not result["ok"]]
        report = {
            "schema_version": "1",
            "harness": "scripts/run_curl_differential.py",
            "curl_version": expected_curl_version,
            "curl_executable": str(curl),
            "mdok_executable": str(mdok),
            "policy": {
                "rows": len(policy),
                "canonical_options": len(options),
                "upstream_short_aliases": all_aliases,
                "explicit_short_aliases": explicit_aliases,
            },
            "summary": {
                "cases": len(results),
                "passed": len(results) - len(failures),
                "failed": len(failures),
                "canonical_cases": sum(result["kind"] == "canonical" for result in results),
                "alias_cases": sum(result["kind"] == "alias" for result in results),
                "repeat_cases": sum(result["kind"] == "repeat" for result in results),
                "repeat_alias_cases": sum(
                    result["kind"] == "repeat-alias" for result in results
                ),
                "negated_cases": sum(result["kind"] == "negated" for result in results),
                "transfer_characteristic_cases": sum(
                    result["kind"] == "transfer-characteristic" for result in results
                ),
                "missing_argument_cases": sum(
                    result["kind"] in {"missing", "missing-alias"} for result in results
                ),
                "unknown_option_cases": sum(result["kind"] == "unknown" for result in results),
                "feature_unavailable_cases": sum(
                    result["curl"]["error_kind"] == "feature-unavailable" for result in results
                ),
            },
            "cases": results,
        }
        rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(rendered, encoding="utf-8")
        print(
            "curl differential: "
            f"{report['summary']['passed']}/{report['summary']['cases']} cases passed; "
            f"{len(failures)} mismatches; "
            f"{len(options)} canonical options, {all_aliases} upstream aliases; "
            f"{report['summary']['feature_unavailable_cases']} feature-unavailable"
        )
        if failures:
            for failure in failures[:20]:
                print(
                    f"FAIL {failure['case_id']}: "
                    + "; ".join(failure["reasons"]),
                    file=sys.stderr,
                )
            if len(failures) > 20:
                print(f"... {len(failures) - 20} more mismatches; see --report", file=sys.stderr)
        return 1 if failures and not args.no_strict else 0
    finally:
        if args.keep_workdir:
            print(f"kept differential workdir: {temporary}", file=sys.stderr)
            temporary_context = None
        if temporary_context is not None:
            temporary_context.cleanup()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"curl differential harness error: {error}", file=sys.stderr)
        raise SystemExit(2)
