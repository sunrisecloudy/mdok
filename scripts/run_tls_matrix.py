#!/usr/bin/env python3
"""Run the portable MDOK HTTPS/session policy matrix on one Tier-1 host.

The runner is intentionally CI-agnostic: invoke it once on each supported
target and retain the JSON result.  It exercises verified HTTPS through the
compatibility path, explicit local insecure TLS through the native path, two
sequential same-origin requests, and the policy rejection cases.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import queue
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from typing import Any, TextIO


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MDOK = ROOT / "target" / "release" / "mdok"
DEFAULT_SERVER = ROOT / "target" / "release" / "test-server"
TIER1_TARGETS = (
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
)
READY_TIMEOUT_SECONDS = 15.0
BUILD_TIMEOUT_SECONDS = 600.0
CASE_TIMEOUT_SECONDS = 60.0


def command_path(path: Path, windows: bool) -> Path:
    if windows and path.suffix.lower() != ".exe":
        return path.with_name(f"{path.name}.exe")
    return path


def run(
    command: list[str],
    *,
    cwd: Path = ROOT,
    env: dict[str, str] | None = None,
    timeout: float | None = None,
) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=cwd,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        command_text = " ".join(command)
        raise RuntimeError(
            f"command timed out after {timeout:g}s: {command_text}"
        ) from error


StreamEvent = tuple[str, str | None]


def _pump_stream(name: str, stream: TextIO, events: queue.Queue[StreamEvent]) -> None:
    try:
        for line in iter(stream.readline, ""):
            events.put((name, line))
    except (OSError, ValueError) as error:
        events.put((name, f"[{name} read error: {error}]\n"))
    finally:
        events.put((name, None))


def _tail(lines: list[str], limit: int = 4000) -> str:
    return "".join(lines)[-limit:]


def wait_ready(
    process: subprocess.Popen[str], timeout: float = READY_TIMEOUT_SECONDS
) -> dict[str, Any]:
    """Read readiness without ever blocking the caller on a child pipe.

    ``select`` cannot portably wait on subprocess pipes on Windows, so each
    pipe gets a daemon reader and the main thread waits on a bounded queue.
    The deadline therefore applies even when the fixture emits no output.
    """

    events: queue.Queue[StreamEvent] = queue.Queue()
    for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
        if stream is None:
            continue
        reader = threading.Thread(
            target=_pump_stream,
            args=(name, stream, events),
            name=f"mdok-fixture-{name}",
            daemon=True,
        )
        reader.start()

    stdout_tail: list[str] = []
    stderr_tail: list[str] = []
    stdout_closed = False
    deadline = time.monotonic() + timeout
    while True:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        try:
            name, line = events.get(timeout=min(remaining, 0.1))
        except queue.Empty:
            if process.poll() is not None:
                break
            continue

        if line is None:
            if name == "stdout":
                stdout_closed = True
            continue
        if name == "stderr":
            stderr_tail.append(line)
            continue

        stdout_tail.append(line)
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        required_keys = ("http_base_url", "https_base_url", "ca_file")
        if isinstance(value, dict) and all(
            isinstance(value.get(key), str) and value[key] for key in required_keys
        ):
            return value

        if process.poll() is not None and stdout_closed:
            break

    if process.poll() is not None:
        details = _tail(stderr_tail) or _tail(stdout_tail)
        raise RuntimeError(
            f"fixture server exited with {process.returncode}: {details.strip()}"
        )
    details = _tail(stderr_tail)
    suffix = f": {details.strip()}" if details.strip() else ""
    raise RuntimeError(f"timed out waiting for mdok-test-server readiness{suffix}")


def write_config(path: Path, ca_file: Path, *, allow_insecure: bool) -> None:
    read_root = json.dumps(str(ca_file.parent))
    path.write_text(
        "\n".join(
            [
                'language = "1"',
                'curl_compat = "8.21"',
                "",
                "[execution]",
                'allowed_schemes = ["https"]',
                'connect_timeout = "5s"',
                'total_timeout = "15s"',
                "",
                "[policy]",
                'allowed_hosts = ["127.0.0.1", "localhost"]',
                f"allowed_read_paths = [{read_root}]",
                f"allow_insecure_tls = {str(allow_insecure).lower()}",
                "",
            ]
        ),
        encoding="utf-8",
    )


def write_document(path: Path, *, insecure: bool, include_cacert: bool) -> None:
    if insecure and include_cacert:
        raise ValueError("TLS matrix document cannot request --insecure and --cacert")
    if insecure:
        option = " --insecure"
    elif include_cacert:
        option = " --cacert {{ca_file}}"
    else:
        option = ""
    request = f"curl{option} {{{{https_base_url}}}}/health"
    path.write_text(
        "\n".join(
            [
                "# TLS matrix",
                "",
                "```curl mdok name=first",
                request,
                "```",
                "",
                "```jmespath mdok check=first",
                "status == `200`",
                "```",
                "",
                "```curl mdok name=second",
                request,
                "```",
                "",
                "```jmespath mdok check=second",
                "status == `200`",
                "```",
                "",
            ]
        ),
        encoding="utf-8",
    )


def run_case(
    binary: Path,
    config: Path,
    document: Path,
    variables: dict[str, str],
    *,
    expect_success: bool,
    name: str,
    expected_error_code: str | None = None,
    expected_steps: tuple[str, ...] = (),
) -> dict[str, Any]:
    command = [str(binary), "--config", str(config), "--json", "test", str(document)]
    for key, value in variables.items():
        command.extend(["--var", f"{key}={value}"])
    completed = run(command, timeout=CASE_TIMEOUT_SECONDS)
    output = f"{completed.stdout}\n{completed.stderr}"
    observed_steps: list[tuple[str, str]] = []
    if expect_success:
        passed = completed.returncode == 0
        if passed and expected_steps:
            try:
                report = json.loads(completed.stdout)
                documents = report["documents"]
                steps = documents[0]["steps"]
                observed_steps = [(step["name"], step["status"]) for step in steps]
            except (IndexError, KeyError, TypeError, json.JSONDecodeError) as error:
                raise RuntimeError(
                    f"TLS case {name} returned an invalid JSON execution report"
                ) from error
            passed = observed_steps == [(step, "passed") for step in expected_steps]
    else:
        passed = completed.returncode != 0 and (
            expected_error_code is None or expected_error_code in output
        )
    if not passed:
        raise RuntimeError(
            f"TLS case {name} failed: exit={completed.returncode}\n{output[-4000:]}"
        )
    return {
        "name": name,
        "expected_success": expect_success,
        **(
            {"expected_error_code": expected_error_code}
            if expected_error_code is not None
            else {}
        ),
        **({"observed_steps": observed_steps} if expected_steps else {}),
        "returncode": completed.returncode,
        "passed": True,
    }


def rustc_host_target() -> str | None:
    try:
        result = run(["rustc", "-vV"], timeout=10.0)
    except (OSError, RuntimeError):
        return None
    if result.returncode != 0:
        return None
    for line in result.stdout.splitlines():
        key, separator, value = line.partition(":")
        if separator and key.strip() == "host" and value.strip():
            return value.strip()
    return None


def fallback_rust_target() -> str:
    system = platform.system()
    machine = platform.machine().lower()
    machine_aliases = {
        "amd64": "x86_64",
        "arm64": "aarch64",
    }
    architecture = machine_aliases.get(machine, machine)
    if system == "Darwin" and architecture in {"aarch64", "x86_64"}:
        return f"{architecture}-apple-darwin"
    if system == "Linux" and architecture in {"aarch64", "x86_64"}:
        return f"{architecture}-unknown-linux-gnu"
    if system == "Windows" and architecture in {"aarch64", "x86_64", "i686"}:
        return f"{architecture}-pc-windows-msvc"
    if system == "FreeBSD" and architecture == "x86_64":
        return "x86_64-unknown-freebsd"
    return f"unknown-{system.lower()}-{machine}"


def default_target() -> str:
    return rustc_host_target() or fallback_rust_target()


def source_revision() -> str | None:
    try:
        result = run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], timeout=10.0)
    except (OSError, RuntimeError):
        return None
    revision = result.stdout.strip()
    return revision if result.returncode == 0 and revision else None


def stop_process(process: subprocess.Popen[str]) -> None:
    if process.poll() is None:
        try:
            process.terminate()
        except OSError:
            pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        try:
            process.kill()
        except OSError:
            pass
        process.wait()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", default=None, help="Tier-1 target label for this host")
    parser.add_argument("--mdok", type=Path, default=DEFAULT_MDOK)
    parser.add_argument("--server", type=Path, default=DEFAULT_SERVER)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--output", type=Path, default=Path("target/tls-matrix.json"))
    args = parser.parse_args()

    target = args.target or default_target()
    windows = os.name == "nt"
    mdok = command_path(args.mdok.resolve(), windows)
    server = command_path(args.server.resolve(), windows)
    if not args.skip_build:
        build = run(
            [
                "cargo",
                "build",
                "--locked",
                "--release",
                "--package",
                "mdok-cli",
                "--package",
                "mdok-test-server",
            ],
            timeout=BUILD_TIMEOUT_SECONDS,
        )
        if build.returncode:
            raise SystemExit(build.stderr or "release build failed")
    if not mdok.is_file() or not server.is_file():
        raise SystemExit(f"missing release binaries: {mdok} and {server}")

    server_process = subprocess.Popen(
        [str(server), "--listen", "127.0.0.1:0", "--tls-listen", "127.0.0.1:0", "--json-ready"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        ready = wait_ready(server_process)
        ca_file = Path(ready["ca_file"])
        with tempfile.TemporaryDirectory(prefix="mdok-tls-matrix-") as directory:
            root = Path(directory)
            config = root / "mdok.toml"
            document = root / "tls.md"
            wrong_ca = root / "wrong-ca.pem"
            wrong_ca.write_text("not a certificate\n", encoding="ascii")

            write_config(config, ca_file, allow_insecure=False)
            write_document(document, insecure=False, include_cacert=True)
            cases = [
                run_case(
                    mdok,
                    config,
                    document,
                    {"https_base_url": ready["https_base_url"], "ca_file": str(ca_file)},
                    expect_success=True,
                    name="verified_https_compatibility_two_steps",
                    expected_steps=("first", "second"),
                ),
                run_case(
                    mdok,
                    config,
                    document,
                    {"https_base_url": ready["https_base_url"], "ca_file": str(wrong_ca)},
                    expect_success=False,
                    name="wrong_ca_rejected",
                    expected_error_code="MDOK-E602",
                ),
            ]

            write_config(config, ca_file, allow_insecure=False)
            write_document(document, insecure=True, include_cacert=False)
            cases.append(
                run_case(
                    mdok,
                    config,
                    document,
                    {"https_base_url": ready["https_base_url"]},
                    expect_success=False,
                    name="insecure_tls_denied_by_default",
                    expected_error_code="MDOK-E602",
                )
            )

            write_config(config, ca_file, allow_insecure=True)
            cases.append(
                run_case(
                    mdok,
                    config,
                    document,
                    {"https_base_url": ready["https_base_url"]},
                    expect_success=True,
                    name="explicit_local_insecure_native_path",
                    expected_steps=("first", "second"),
                )
            )
    finally:
        stop_process(server_process)

    result = {
        "schema": "mdok-tls-matrix-v1",
        "source_revision": source_revision(),
        "target": target,
        "tier1_targets": list(TIER1_TARGETS),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "cases": cases,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"TLS matrix passed: {len(cases)} cases on {target}; wrote {args.output}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        raise SystemExit(f"run_tls_matrix.py: {error}") from error
