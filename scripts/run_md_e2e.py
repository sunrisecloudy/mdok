#!/usr/bin/env python3
"""Run the checked-in Markdown E2E workflows against the local fixture server.

The runner owns fixture lifecycle and readiness parsing. Every HTTP interaction
still goes through ``mdok test``; this script does not make API requests itself.
"""

from __future__ import annotations

import argparse
import json
import queue
import subprocess
import tempfile
import threading
import time
from pathlib import Path
from typing import Any, TextIO


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MDOK = ROOT / "target" / "debug" / "mdok"
DEFAULT_SERVER = ROOT / "target" / "debug" / "test-server"
DEFAULT_MANIFEST = ROOT / "tests" / "e2e" / "manifest.txt"
READY_TIMEOUT_SECONDS = 15.0
BUILD_TIMEOUT_SECONDS = 600.0
CASE_TIMEOUT_SECONDS = 90.0


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


def wait_ready(process: subprocess.Popen[str], timeout: float = READY_TIMEOUT_SECONDS) -> dict[str, str]:
    """Read one valid readiness JSON object without blocking forever on pipes."""

    events: queue.Queue[StreamEvent] = queue.Queue()
    for name, stream in (("stdout", process.stdout), ("stderr", process.stderr)):
        if stream is None:
            continue
        threading.Thread(
            target=_pump_stream,
            args=(name, stream, events),
            name=f"mdok-e2e-{name}",
            daemon=True,
        ).start()

    stdout_tail: list[str] = []
    stderr_tail: list[str] = []
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            name, line = events.get(timeout=min(deadline - time.monotonic(), 0.1))
        except queue.Empty:
            if process.poll() is not None:
                break
            continue
        if line is None:
            continue
        if name == "stderr":
            stderr_tail.append(line)
            continue
        stdout_tail.append(line)
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        required = ("http_base_url", "https_base_url", "ca_file")
        if isinstance(payload, dict) and all(
            isinstance(payload.get(key), str) and payload[key] for key in required
        ):
            return {key: str(payload[key]) for key in ("http_base_url", "https_base_url", "proxy_url", "ca_file") if key in payload}

    if process.poll() is not None:
        details = _tail(stderr_tail) or _tail(stdout_tail)
        raise RuntimeError(f"fixture server exited with {process.returncode}: {details.strip()}")
    details = _tail(stderr_tail)
    suffix = f": {details.strip()}" if details.strip() else ""
    raise RuntimeError(f"timed out waiting for fixture readiness{suffix}")


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


def run(command: list[str], *, timeout: float | None = None) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f"command timed out after {timeout:g}s: {' '.join(command)}") from error


def load_manifest(path: Path) -> list[Path]:
    if not path.is_file():
        raise RuntimeError(f"missing E2E manifest: {path}")
    documents: list[Path] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        entry = line.strip()
        if not entry or entry.startswith("#"):
            continue
        document = (ROOT / entry).resolve()
        try:
            document.relative_to(ROOT)
        except ValueError as error:
            raise RuntimeError(f"manifest line {line_number} escapes repository: {entry}") from error
        if document.suffix != ".md" or not document.is_file():
            raise RuntimeError(f"manifest line {line_number} is not an existing Markdown file: {entry}")
        documents.append(document)
    if not documents:
        raise RuntimeError(f"E2E manifest is empty: {path}")
    return documents


def write_config(path: Path, ca_file: Path) -> None:
    """Give this run only the loopback and CA read permissions it needs."""

    path.write_text(
        "\n".join(
            [
                'language = "1"',
                'curl_compat = "8.21"',
                "",
                "[execution]",
                'allowed_schemes = ["http", "https"]',
                'connect_timeout = "5s"',
                'total_timeout = "30s"',
                "",
                "[policy]",
                'allowed_hosts = ["127.0.0.1", "localhost"]',
                f"allowed_read_paths = [{json.dumps(str(ca_file.parent))}]",
                "",
            ]
        ),
        encoding="utf-8",
    )


def command_for(binary: Path, config: Path, document: Path, ready: dict[str, str], mode: str) -> list[str]:
    command = [
        str(binary),
        "--config",
        str(config),
        "--allow-host",
        "127.0.0.1",
        "--var",
        f"base_url={ready['http_base_url']}",
        "--var",
        f"https_base_url={ready['https_base_url']}",
        "--var",
        f"ca_file={ready['ca_file']}",
        "--json",
        mode,
        str(document),
    ]
    return command


def validate_report(completed: subprocess.CompletedProcess[str], document: Path, mode: str) -> None:
    if completed.returncode != 0:
        output = f"{completed.stdout}\n{completed.stderr}"
        raise RuntimeError(f"{mode} failed for {document.relative_to(ROOT)} (exit {completed.returncode}):\n{output[-4000:]}")
    try:
        report: dict[str, Any] = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{mode} returned invalid JSON for {document.relative_to(ROOT)}: {error}") from error
    documents = report.get("documents")
    if not isinstance(documents, list) or len(documents) != 1:
        raise RuntimeError(f"{mode} returned an unexpected document count for {document.relative_to(ROOT)}")
    if mode == "test" and documents[0].get("status") != "passed":
        raise RuntimeError(f"test report did not pass for {document.relative_to(ROOT)}: {json.dumps(documents[0], sort_keys=True)[:4000]}")


def run_document(binary: Path, config: Path, document: Path, ready: dict[str, str]) -> None:
    lint = run(command_for(binary, config, document, ready, "lint"), timeout=CASE_TIMEOUT_SECONDS)
    validate_report(lint, document, "lint")
    test = run(command_for(binary, config, document, ready, "test"), timeout=CASE_TIMEOUT_SECONDS)
    validate_report(test, document, "test")
    print(f"PASS {document.relative_to(ROOT)}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_MDOK)
    parser.add_argument("--server", type=Path, default=DEFAULT_SERVER)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()

    binary = args.binary.resolve()
    server = args.server.resolve()
    manifest = args.manifest.resolve()
    documents = load_manifest(manifest)

    if not args.skip_build and (not binary.is_file() or not server.is_file()):
        build = run(
            ["cargo", "build", "--locked", "--package", "mdok-cli", "--package", "mdok-test-server"],
            timeout=BUILD_TIMEOUT_SECONDS,
        )
        if build.returncode:
            raise RuntimeError(f"E2E build failed:\n{build.stderr[-4000:]}")
    if not binary.is_file() or not server.is_file():
        raise RuntimeError(f"missing E2E binaries: {binary} and {server}")

    process = subprocess.Popen(
        [str(server), "--listen", "127.0.0.1:0", "--tls-listen", "127.0.0.1:0", "--json-ready"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        ready = wait_ready(process)
        with tempfile.TemporaryDirectory(prefix="mdok-e2e-") as temporary:
            config = Path(temporary) / "mdok.toml"
            write_config(config, Path(ready["ca_file"]).resolve())
            for document in documents:
                run_document(binary, config, document, ready)
    finally:
        stop_process(process)

    print(f"Markdown E2E passed: {len(documents)} workflows")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        raise SystemExit(f"run_md_e2e.py: {error}") from error
