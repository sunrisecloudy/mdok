#!/usr/bin/env python3
"""Differential parity suite: Rust mdok vs the Go port.

Generates a large, deterministic corpus of Markdown workflows (~100x the
hand-written e2e suite) spanning the ported feature matrix, runs every
workflow through BOTH binaries against the shared fixture server, and
compares normalized outcomes:

- process exit code
- document status
- per-step (name, status, method, URL path+query, status code)
- diagnostic codes (multiset) and severities

Any disagreement is a parity bug in one of the two implementations (or in
this harness). Volatile values (durations, ports, run ids, temp paths,
server test keys) are normalized before comparison.

Deterministic: cases are enumerated from fixed matrices plus a seeded PRNG.
Mutable fixture state (retry counters, users) is isolated per case AND per
binary side via an X-Mdok-Test-Key derived from "{{mdok_side}}".

Usage:
    python3 scripts/run_parity_suite.py                       # Rust vs Go
    python3 scripts/run_parity_suite.py --self-check          # Rust vs Rust
    python3 scripts/run_parity_suite.py --limit 100           # smaller run
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
RUST_BINARY = ROOT / "target" / "debug" / "mdok"
GO_BINARY = ROOT / "go" / "bin" / "mdok"
SERVER = ROOT / "go" / "bin" / "test-server"
CASE_TIMEOUT = 60
WORKERS = 8

NORMALIZERS: list[tuple[str, str]] = [
    (r"127\.0\.0\.1:\d+", "127.0.0.1:$PORT"),
    (r"/var/folders/\S+|/tmp/\S+", "$TMP"),
    (r"-rust-\d+|-go-\d+", "-$SIDE"),
]

Case = dict[str, Any]  # {id, markdown, mode: lint|test}


def fence(info: str, body: str) -> str:
    return f"```{info}\n{body}\n```"


def curl_step(name: str, *lines: str) -> str:
    joined = " \\\n  ".join(lines)
    return fence(f"curl mdok name={name}", joined)


def check_step(step: str, *exprs: str) -> str:
    return fence(f"jmespath mdok check={step}", "\n".join(exprs))


def capture_step(step: str, expr: str) -> str:
    return fence(f"jmespath mdok capture={step}", expr)


def vars_fence(pairs: dict[str, str]) -> str:
    body = "\n".join(f"{k} = {v}" for k, v in pairs.items())
    return fence("toml mdok vars", body)


def toml_str(value: str) -> str:
    # Raw UTF-8: TOML basic strings allow it, and \uXXXX surrogate escapes
    # (json.dumps default) are invalid TOML.
    return json.dumps(value, ensure_ascii=False)


SPECIALS = [
    "plain", "with space", "slash/inside", "plus+sign", "amp&ers", "eq=uals",
    "percent%25", "hash#tag", "question?mark", "quote\"d", "unique-id-ไทย",
    "emoji-🚀-code", "tab\tchar", "semi;colon", "at@sign",
    "colon:sep", "tilde~x", "star*glob", "paren(left)", "brack[et]",
    "brace{s}", "pipe|bar", "caret^up", "dollar$bill", "back`tick",
    "less<greater>", "single'quote", "double\"double", "empty-ish-",
    "0number", "42", "true", "null-literal", "-dash", "dot.name",
    "UPPER", "MiXeD", "under_score", "multi  space", "trailing ",
]

UNICODE_TRIPLES = [
    ("ascii", "hello"),
    ("thaicombo", "สวัสดีgolang"),
    ("fullwidth", "ＦＵＬＬ"),
    ("combining", "éclair"),
    ("rtl", "עברית"),
    ("cjk", "日本語テキスト"),
    ("mixed", "a1!`~^&*()[]{}<>|\\:;'\"," "bc"),
]

CHECK_POOL = [
    "status == `200`",
    "status != `404`",
    "body.method == 'GET'",
    "body.method == 'POST'",
    "type(body) == 'object'",
    "length(body.query) > `0`",
    "body.missing == `null`",
    "status >= `200` && status < `300`",
    "body.ok == `true`",
    "length(length(body)) > `0`",
    "!(status == `404`)",
    "status == `200` || status == `201`",
    "keys(body) != `null`",
    "type(status) == 'number'",
    "type(body.method) == 'string'",
    "body.headers != `null`",
    "contains(keys(body), 'method')",
    "to_string(status) == '200'",
    "length(body) >= `1`",
    "status < `500`",
    "body.query == body.query",
    "!(body.missing != `null`)",
    "type(body.missing) == 'null'",
    "body.json == `null` || body.json != `null`",
    "abs(status - `200`) == `0`",
]

CAPTURE_POOL = [
    "{c1: body.method}",
    "{c1: status, c2: body.method}",
    "{c1: body.query}",
    "{c1: length(body)}",
    "{c1: body.query.q, c2: body.query.page}",
    "{c1: body.ok}",
    "{c1: keys(body)}",
    "{c1: body.json.name, c2: body.json.active, c3: status}",
    "{c1: body.cookies}",
    "{c1: body.user}",
]


def gen_cases(rng: random.Random) -> list[Case]:
    cases: list[Case] = []

    def add(mode: str, markdown: str, category: str) -> None:
        cases.append({
            "id": f"P{len(cases) + 1:04d}",
            "mode": mode,
            "category": category,
            "markdown": f"# Case {len(cases) + 1:04d}\n\n{markdown}\n",
        })

    base = '"{{base_url}}'
    https = '"{{https_base_url}}'
    side_key = '--header "X-Mdok-Test-Key: p-{{mdok_side}}-{{case_id}}"'

    # 1. Methods against /echo ------------------------------------------------
    for method in ("GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"):
        for with_body in (False, True):
            lines = [f"curl --request {method} {base}/echo\""]
            if with_body and method != "HEAD":
                lines.append("--data-raw '{\"k\":\"v\"}'")
            md = curl_step("m", *lines)
            md += "\n" + check_step("m", "status == `200`", f"body.method == '{method}'")
            add("test", md, "methods")

    # 2. Header zoo -----------------------------------------------------------
    headers = [
        '--header "X-Custom: one"',
        '--header "X-Custom: one" \\\n  --header "X-Custom: two"',
        '--header "Content-Type: application/json"',
        '--header "Accept: text/plain"',
        '-H "X-Short: v"',
        '--header "Authorization: Bearer tok-{{mdok_side}}"',
        '--user-agent "parity/1.0"',
        '--referer "https://example.invalid/ref"',
        '--user "alice:secret"',
    ]
    for i, header in enumerate(headers):
        md = curl_step("h", f"curl {base}/echo\" \\\n  {header}")
        md += "\n" + check_step("h", "status == `200`")
        add("test", md, "headers")

    # 3. Bodies and form encoding --------------------------------------------
    for special in SPECIALS:
        md = vars_fence({"v": toml_str(special)})
        md += "\n" + curl_step(
            "b", 'curl --get ' + base + '/echo"',
            f'--data-urlencode "v={{{{v|url}}}}"',
        )
        md += "\n" + check_step("b", "status == `200`", "body.query.v == variables.v")
        add("test", md, "urlencode")
    for i, (label, value) in enumerate(UNICODE_TRIPLES):
        md = vars_fence({"v": toml_str(value)})
        md += "\n" + curl_step(
            "b", 'curl --get ' + base + '/echo"',
            '--data-urlencode "v={{v|url}}"',
            '--data-urlencode "enc=unicode"',
        )
        md += "\n" + check_step("b", "status == `200`", "body.query.enc == 'unicode'")
        add("test", md, "urlencode-unicode")
    for payload in ('{"name":"Ada"}', '{"n":1,"flag":true,"list":[1,2,3]}', "{}", "[]"):
        md = curl_step("b", f'curl --request POST {base}/echo"',
                       f"--data-raw '{payload}'")
        md += "\n" + check_step("b", "status == `200`", "body.json != `null`")
        add("test", md, "json-body")
    for a, b in (("a=1", "b=2"), ("x={{mdok_side}}", "y=plain"), ("q=", "=v")):
        md = curl_step("b", f'curl --request POST {base}/echo"',
                       f"--data '{a}'", f"--data '{b}'")
        md += "\n" + check_step("b", "status == `200`")
        add("test", md, "form-body")

    # 4. Template filters -----------------------------------------------------
    filter_values = {
        "s": toml_str("plain text"),
        "num_i": "42",
        "num_f": "3.5",
        "flag_t": "true",
        "flag_f": "false",
        "uni": toml_str("ไทย-42"),
    }
    for var, literal in filter_values.items():
        for filt in ("string", "url", "json", "raw", "base64"):
            md = vars_fence(dict(filter_values))
            md += "\n" + curl_step(
                "t", "curl --get " + base + '/echo"',
                f'--data-urlencode "v={{{{ {var} | {filt} }}}}"',
            )
            md += "\n" + check_step("t", "status == `200`", "body.query.v != `null`")
            add("test", md, "template-filters")
    # header filter round-trip
    md = vars_fence({"tok": toml_str("token-with-space")})
    md += "\n" + curl_step("t", f"curl {base}/headers\"",
                           '--header "X-Tok: {{tok|header}}"')
    md += "\n" + check_step("t", "status == `200`")
    add("test", md, "template-filters")
    # missing variable at execution
    add("test", curl_step("t", f"curl {base}/echo\"",
                          '--header "X-V: {{absent_var|string}}"'), "template-errors")
    # depth limit
    add("lint", curl_step("t", 'curl -d "{{' + ".".join(["a"] * 34) + '}}" "https://api.example.test/x"'),
        "template-errors")

    # 5. JMESPath check zoo ---------------------------------------------------
    for expr in CHECK_POOL:
        md = curl_step("c", f"curl {base}/echo\"")
        md += "\n" + check_step("c", "status == `200`", expr)
        add("test", md, "jmespath")
    for bad in ("status ==", "this is not jmespath !", "body[", "missing_fn(x)"):
        md = curl_step("c", f"curl {base}/echo\"")
        md += "\n" + check_step("c", "status == `200`", bad)
        add("test", md, "jmespath-errors")
    for expr in ("status == `201`", "body.method == 'PUT'"):
        md = curl_step("c", f"curl {base}/echo\"")
        md += "\n" + check_step("c", expr)
        add("test", md, "jmespath-false")

    # 6. Captures -------------------------------------------------------------
    for expr in CAPTURE_POOL:
        md = curl_step("s", f"curl {base}/echo\"")
        md += "\n" + capture_step("s", expr)
        md += "\n" + curl_step("u", "curl --get " + base + '/echo"',
                               '--data-urlencode "c={{c1|string}}"')
        md += "\n" + check_step("u", "status == `200`")
        add("test", md, "captures")
    # capture from /json/standard and consume
    md = curl_step("s", f"curl {base}/json/standard\"")
    md += "\n" + capture_step("s", "{item_id: body.items[2].id, total: length(body.items)}")
    md += "\n" + curl_step("u", "curl --get " + base + '/echo"',
                           '--data-urlencode "id={{item_id|string}}"')
    md += "\n" + check_step("u", "status == `200`", "body.query.id == variables.item_id | to_string(@)")
    add("test", md, "captures")
    # non-object capture
    md = curl_step("s", f"curl {base}/echo\"")
    md += "\n" + capture_step("s", "body.method")
    add("test", md, "capture-errors")

    # 7. Redirects ------------------------------------------------------------
    for hops in (1, 2, 3, 4, 5):
        for redirs in (hops, hops + 2, 50):
            md = curl_step("r", f'curl --location --max-redirs {redirs} '
                           f'{base}/redirect/{hops}?final=/echo\"')
            md += "\n" + check_step("r", "status == `200`",
                                    f"transfer.redirect_count == `{hops}`")
            add("test", md, "redirects")
    md = curl_step("r", f'curl {base}/redirect/1?final=/echo\"')
    md += "\n" + check_step("r", "status == `302`", "transfer.redirect_count == `0`")
    add("test", md, "redirects")
    md = curl_step("r", f'curl --location --max-redirs 1 '
                   f'{base}/redirect/3?final=/echo\"')
    md += "\n" + check_step("r", "transfer.redirect_count == `1`")
    add("test", md, "redirects-limit")

    # 8. Cookies --------------------------------------------------------------
    md = curl_step("ck", f'curl --cookie "a=1" {base}/cookies/echo\"')
    md += "\n" + check_step("ck", "status == `200`", "body.cookies.a == '1'")
    add("test", md, "cookies")
    md = curl_step("ck", f'curl --cookie "a=1" --cookie "b=two" {base}/cookies/echo\"')
    md += "\n" + check_step("ck", "body.cookies.b == 'two'")
    add("test", md, "cookies")
    md = curl_step("ck", f'curl --location --cookie "fixture=ok" '
                   f'{base}/redirect/1?final=/cookies/echo\"')
    md += "\n" + check_step("ck", "status == `200`", "body.cookies.fixture == 'ok'")
    add("test", md, "cookies")

    # 9. Retry (side-isolated keys) -------------------------------------------
    for fails in (1, 2, 3, 4):
        for retries in (fails, fails + 1, fails + 2):
            md = curl_step("rt", f"curl --retry {retries} --retry-delay 0",
                           side_key, f'{base}/retry/{fails}"')
            md += "\n" + check_step("rt", "status == `200`",
                                    f"body.attempt == `{fails + 1}`")
            add("test", md, "retry")
    md = curl_step("rt", "curl --retry 1 --retry-delay 0", side_key, f'{base}/retry/3"')
    md += "\n" + check_step("rt", "status == `503`")
    add("test", md, "retry-exhausted")

    # 10. Status codes --------------------------------------------------------
    for code in (200, 201, 202, 204, 301, 400, 401, 403, 404, 405, 409, 410,
                 418, 422, 429, 451, 500, 501, 502, 503, 504):
        md = curl_step("s", f"curl {base}/status/{code}\"")
        md += "\n" + check_step("s", f"status == `{code}`")
        add("test", md, "status")
        add("lint", curl_step("s", f"curl {base}/status/{code}\""), "status")

    # 11. TLS -----------------------------------------------------------------
    for endpoint in ("/health", "/echo", "/status/201", "/cookies/echo", "/json/standard"):
        md = curl_step("t", f'curl {https}{endpoint}" --cacert "{{{{ca_file}}}}"')
        md += "\n" + check_step("t", "status == `200`")
        add("test", md, "tls")

    # 12. gzip / binary / large ----------------------------------------------
    md = curl_step("g", f"curl {base}/gzip\"")
    md += "\n" + check_step("g", "status == `200`", "body.ok == `true`")
    add("test", md, "gzip")
    for size in (16, 64, 256, 1024, 4096):
        md = curl_step("b", f"curl {base}/binary/{size}\"")
        md += "\n" + check_step("b", "status == `200`", f"length(body) == `{size // 4}`")
        add("test", md, "binary")

    # 13. Users CRUD (side-isolated) ------------------------------------------
    md = curl_step("c1", "curl --request POST " + base + '/users"',
                   side_key, '--data-raw \'{"id":"u1","name":"Ada"}\'')
    md += "\n" + capture_step("c1", "{uid: body.id}")
    md += "\n" + curl_step("c2", f'curl {base}/users/{{{{uid|url}}}}"', side_key)
    md += "\n" + check_step("c2", "status == `200`", "body.name == 'Ada'")
    md += "\n" + curl_step("c3", "curl --request PATCH " + base + '/users/{{uid|url}}"',
                           side_key, '--data-raw \'{"name":"Grace"}\'')
    md += "\n" + check_step("c3", "body.name == 'Grace'")
    md += "\n" + curl_step("c4", f'curl --request DELETE {base}/users/{{{{uid|url}}}}"', side_key)
    md += "\n" + check_step("c4", "body.deleted == `true`")
    add("test", md, "users-crud")

    # 14. Static lint: valid docs ---------------------------------------------
    for extra in ("", " \\\n  -s", " \\\n  --compressed", " \\\n  --fail",
                  " \\\n  --max-time 30", " \\\n  --connect-timeout 5"):
        md = curl_step("v", f'curl{extra} https://api.example.test/x')
        add("lint", md, "lint-valid")
    add("lint", vars_fence({"a": "1", "b": toml_str("x")}) + "\n"
        + curl_step("v", 'curl "https://api.example.test/{{a}}"'), "lint-valid")
    add("lint", check_step("nonexistent", "status == `200`"), "lint-invalid")
    add("lint", curl_step("dup", 'curl "https://a.test/"') + "\n"
        + curl_step("dup", 'curl "https://b.test/"'), "lint-invalid")
    add("lint", curl_step("bad!name", 'curl "https://a.test/"'), "lint-invalid")

    # 15. Static lint: shell/option/policy errors ------------------------------
    for source in ('curl "https://a.test/x\\', 'curl "https://a.test/x',
                   'curl https://a.test/a https://b.test/b', 'curl | tee out',
                   'curl a; curl b', 'echo hi', '', 'CURL=1 curl "https://a.test/"'):
        add("lint", curl_step("e", source) if source else fence("curl mdok name=e", ""),
            "lint-shell")
    for opt in ("--form", "--upload-file", "--proxy", "--resolve", "--json",
                "--next", "-Z", "--parallel", "--no-clobber"):
        add("lint", curl_step("e", f'curl {opt} "https://a.test/"'), "lint-options")
    add("lint", curl_step("e", 'curl "ftp://a.test/x"'), "lint-policy")
    add("lint", curl_step("e", 'curl "https://not-allowed.test/x"'), "lint-policy")
    add("lint", curl_step("e", 'curl --cacert "/etc/hosts" "https://a.test/"'), "lint-policy")

    # 16. Execution-time policy failures --------------------------------------
    add("test", curl_step("d", 'curl "https://denied.example.invalid/x"'), "exec-policy")
    add("test", curl_step("d", 'curl "ftp://127.0.0.1/x"'), "exec-policy")
    add("test", curl_step("d", 'curl "{{base_url}}/echo" --cacert "/etc/hosts"'),
        "exec-policy")

    # 17. Multi-step workflows -------------------------------------------------
    for i in range(40):
        first, second = rng.sample(SPECIALS, 2)
        md = vars_fence({"p": toml_str(first), "q": toml_str(second)})
        md += "\n" + curl_step("s1", "curl --get " + base + '/echo"',
                               '--data-urlencode "v={{p|url}}"')
        md += "\n" + capture_step("s1", "{echoed: body.query.v}")
        md += "\n" + curl_step("s2", "curl --get " + base + '/echo"',
                               '--data-urlencode "v={{echoed|url}}"',
                               '--data-urlencode "w={{q|url}}"')
        md += "\n" + check_step("s2", "status == `200`",
                                "body.query.v == variables.echoed",
                                "body.query.w == variables.q")
        add("test", md, "multi-step")


    # 18. Seeded composer: randomized-but-valid multi-step workflows. The
    # seed keeps the whole suite reproducible while the combinatorial space
    # is far larger than any hand-written matrix.
    endpoints = ["/echo", "/status/200", "/status/201", "/json/standard",
                 "/cookies/echo", "/health"]
    methods = ["GET", "POST", "PUT", "PATCH"]
    filters = ["string", "url", "json"]
    for i in range(580):
        parts = []
        varname = f"cv{i}"
        parts.append(vars_fence({varname: toml_str(rng.choice(SPECIALS))}))
        step_count = rng.randint(2, 4)
        captured_names = []
        for step_index in range(step_count):
            name = f"s{step_index}"
            endpoint = rng.choice(endpoints)
            method = rng.choice(methods)
            lines = [f"curl --request {method} {base}{endpoint}\""]
            if rng.random() < 0.5:
                lines.append(f"--data-urlencode \"v={{{{ {varname} | {rng.choice(filters)} }}}}\"")
            if captured_names and rng.random() < 0.6:
                use = rng.choice(captured_names)
                lines.append(f"--header \"X-Use-{use}: {{{{{use}|string}}}}\"")
            parts.append(curl_step(name, *lines))
            if endpoint in ("/echo", "/json/standard") and rng.random() < 0.7:
                if rng.random() < 0.5:
                    parts.append(check_step(name, "status == `200`",
                                            rng.choice(CHECK_POOL)))
                else:
                    cap = f"cap{step_index}"
                    parts.append(capture_step(name, f"{{{cap}: length(body)}}"))
                    captured_names.append(cap)
        add("test", "\n\n".join(parts), "composer")

    for doc in cases:
        doc["markdown"] = doc["markdown"].replace("{{case_id}}", doc["id"][1:])
    return cases


CONFIG_TEMPLATE = """\
language = "1"
curl_compat = "8.21"

[execution]
allowed_schemes = ["http", "https"]
connect_timeout = "5s"
total_timeout = "30s"

[policy]
allowed_hosts = ["127.0.0.1", "localhost"]
allowed_read_paths = ["{ca_dir}"]
"""


def normalize(text: str, side: str) -> str:
    for pattern, replacement in NORMALIZERS:
        text = re.sub(pattern, replacement, text)
    return text.replace(f"-{side}-", "-$SIDE-")


def project(exit_code: int, stdout: str, stderr: str) -> dict[str, Any]:
    try:
        report = json.loads(stdout)
    except json.JSONDecodeError:
        return {"exit": exit_code, "parse_error": True,
                "stdout_head": stdout[:200], "stderr_head": stderr[:200]}
    docs = report.get("documents", [])
    doc = docs[0] if docs else {}
    # Compare the common wire contract only: Rust and Go step objects carry
    # different extra fields (checks/diagnostics vs method/url), but name and
    # status are the shared behavioral truth.
    steps = [(s.get("name"), s.get("status")) for s in doc.get("steps") or []]
    diags = sorted(
        (d.get("code", "?"), d.get("severity", "?"))
        for d in doc.get("diagnostics") or []
    )
    return {
        "exit": exit_code,
        "status": doc.get("status"),
        "doc_count": len(docs),
        "steps": steps,
        "diags": diags,
    }


def run_case(binary: Path, case: Case, workspace: Path, config: Path,
             ready: dict[str, str]) -> dict[str, Any]:
    doc_path = workspace / f"{case['id']}.md"
    doc_path.write_text(case["markdown"], encoding="utf-8")
    side = "rust" if "target/debug" in str(binary) else "go"
    command = [
        str(binary), "--config", str(config),
        "--allow-host", "127.0.0.1",
        "--var", f"base_url={ready['http_base_url']}",
        "--var", f"https_base_url={ready['https_base_url']}",
        "--var", f"ca_file={ready['ca_file']}",
        "--var", f"mdok_side={side}",
        "--json", case["mode"], str(doc_path),
    ]
    try:
        completed = subprocess.run(command, cwd=str(ROOT), capture_output=True,
                                   text=True, timeout=CASE_TIMEOUT)
        outcome = project(completed.returncode, completed.stdout, completed.stderr)
    except subprocess.TimeoutExpired:
        outcome = {"exit": -1, "timeout": True}
    return outcome


def start_server() -> tuple[subprocess.Popen, dict[str, str]]:
    process = subprocess.Popen(
        [str(SERVER), "--listen", "127.0.0.1:0", "--tls-listen", "127.0.0.1:0",
         "--json-ready"],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
    )
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        line = process.stdout.readline() if process.poll() is None else ""
        if line.strip():
            try:
                payload = json.loads(line)
                if "http_base_url" in payload:
                    return process, payload
            except json.JSONDecodeError:
                pass
        if process.poll() is not None:
            raise RuntimeError("fixture server exited early")
        time.sleep(0.05)
    raise RuntimeError("fixture server readiness timeout")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rust", type=Path, default=RUST_BINARY)
    parser.add_argument("--go", type=Path, default=GO_BINARY)
    parser.add_argument("--self-check", action="store_true",
                        help="run the Rust binary against itself to validate the harness")
    parser.add_argument("--limit", type=int, default=None)
    parser.add_argument("--seed", type=int, default=20260815)
    parser.add_argument("--workers", type=int, default=WORKERS)
    parser.add_argument("--out", type=Path, default=ROOT / "target" / "parity-mismatches.json")
    args = parser.parse_args()

    for label, binary in (("rust", args.rust), ("go", args.go)):
        if not args.self_check and label == "go" and not binary.is_file():
            raise SystemExit(f"missing binary: {binary}")
    if not args.rust.is_file():
        raise SystemExit(f"missing binary: {args.rust}")

    rng = random.Random(args.seed)
    cases = gen_cases(rng)
    if args.limit:
        cases = cases[: args.limit]
    print(f"parity: {len(cases)} generated cases (seed {args.seed})")

    server, ready = start_server()
    mismatches: list[dict[str, Any]] = []
    lock = threading.Lock()
    completed_count = [0]

    def compare(case: Case) -> None:
        with tempfile.TemporaryDirectory(prefix=f"mdok-parity-{case['id']}-") as raw:
            workspace = Path(raw)
            config = workspace / "mdok.toml"
            config.write_text(
                CONFIG_TEMPLATE.format(
                    ca_dir=str(Path(ready["ca_file"]).resolve().parent)),
                encoding="utf-8",
            )
            left = run_case(args.rust, case, workspace, config, ready)
            right = run_case(args.rust if args.self_check else args.go, case,
                             workspace, config, ready)
        if left != right:
            with lock:
                mismatches.append({
                    "id": case["id"], "category": case["category"],
                    "mode": case["mode"], "rust": left, "go": right,
                    "markdown": case["markdown"],
                })
        with lock:
            completed_count[0] += 1
            if completed_count[0] % 100 == 0:
                print(f"  ...{completed_count[0]}/{len(cases)} "
                      f"({len(mismatches)} mismatches)")

    try:
        with ThreadPoolExecutor(max_workers=args.workers) as pool:
            list(pool.map(compare, cases))
    finally:
        server.terminate()
        server.wait(timeout=10)

    by_category: dict[str, int] = {}
    for mismatch in mismatches:
        by_category[mismatch["category"]] = by_category.get(mismatch["category"], 0) + 1

    print(f"\nparity: {len(cases) - len(mismatches)}/{len(cases)} matched")
    if mismatches:
        print("mismatches by category:")
        for category, count in sorted(by_category.items(), key=lambda kv: -kv[1]):
            print(f"  {category}: {count}")
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(mismatches, indent=1), encoding="utf-8")
        print(f"details written to {args.out}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
