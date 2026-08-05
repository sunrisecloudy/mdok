#!/usr/bin/env python3
"""Measure deterministic normal/intense MDOK workloads with wall time and RSS.

The harness intentionally uses only the Python standard library.  It measures
the release CLI against a local HTTP fixture and runs lint, plan, and test so
planning and execution costs are visible separately.
"""

from __future__ import annotations

import argparse
import json
import math
import platform
import re
import resource
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "target" / "release" / "mdok"
RSS_MAC_RE = re.compile(r"^\s*(\d+)\s+maximum resident set size", re.I | re.M)
RSS_LINUX_RE = re.compile(r"maximum resident set size \(kbytes\):\s*(\d+)", re.I)

WORKLOADS = {
    "normal": {"steps": 10, "prose_bytes": 8 * 1024},
    "intense": {"steps": 48, "prose_bytes": 128 * 1024},
}


def markdown_source(label: str, steps: int, prose_bytes: int, endpoint: str) -> str:
    filler = (
        "The benchmark document contains ordinary API notes, examples, and "
        "response guidance.\n"
    )
    source = [
        "# MDOK benchmark document\n\n",
        "```toml mdok vars\n",
        'request_id = "bench-request"\n\n[metadata]\nowner = "performance"\n',
        "```\n\n",
    ]
    for index in range(steps):
        source.extend(
            [
                f"## API step {index}\n\n",
                f"```curl mdok name=step_{index}\n",
                "curl --request POST "
                "--header 'X-Mdok-Request: {{request_id|header}}' "
                f"--header 'X-Mdok-Step: {index}' "
                f"--data '{{\"step\":{index},\"workload\":\"{label}\"}}' "
                f"{endpoint}/api/{index}\n",
                "```\n\n",
                f"```jmespath mdok check=step_{index}\n",
                "status == `200`\n",
                "```\n\n",
                f"```jmespath mdok capture=step_{index}\n",
                f"{{response_id_{index}: body.id}}\n",
                "```\n\n",
            ]
        )
    document = "".join(source)
    while len(document) < prose_bytes:
        document += filler
    return document


def discovery_source(index: int, endpoint: str, target_bytes: int = 2 * 1024) -> str:
    source = (
        f"# Discovery document {index}\n\n"
        f"```curl mdok name=step_{index}\n"
        f"curl {endpoint}/discovery/{index}\n"
        "```\n\n"
    )
    filler = "Discovery workload prose keeps each input near the PRD's 2 KiB target.\n"
    while len(source) < target_bytes:
        source += filler
    return source


class FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    response_body = b'{"id":"bench-response","ok":true}'

    def do_GET(self) -> None:  # noqa: N802 - stdlib handler API
        self._respond()

    def do_POST(self) -> None:  # noqa: N802 - stdlib handler API
        length = int(self.headers.get("Content-Length", "0"))
        if length:
            self.rfile.read(length)
        self._respond()

    def _respond(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(self.response_body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(self.response_body)

    def log_message(self, _format: str, *_args: Any) -> None:
        return


class FixtureServer:
    def __enter__(self) -> str:
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        return f"http://127.0.0.1:{self.server.server_port}"

    def __exit__(self, *_args: Any) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)


def parse_rss(stderr: str, darwin: bool) -> int | None:
    match = (RSS_MAC_RE if darwin else RSS_LINUX_RE).search(stderr)
    if match is None:
        return None
    # macOS reports bytes; Linux reports KiB.
    return int(match.group(1)) if darwin else int(match.group(1)) * 1024


def child_rss_bytes() -> int:
    value = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    # macOS reports bytes; Linux reports KiB.
    return int(value if sys.platform == "darwin" else value * 1024)


def measure(command: list[str]) -> dict[str, Any]:
    time_binary = "/usr/bin/time" if Path("/usr/bin/time").exists() else None
    darwin = platform.system() == "Darwin"
    wrapped = command
    if time_binary:
        wrapped = [time_binary, "-l" if darwin else "-v", *command]
    rss_before = child_rss_bytes()
    started = time.perf_counter()
    completed = subprocess.run(
        wrapped,
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    elapsed = time.perf_counter() - started
    rss = parse_rss(completed.stderr, darwin) if time_binary else None
    if rss is None:
        rss = max(0, child_rss_bytes() - rss_before)
    return {
        "seconds": elapsed,
        "rss_bytes": rss,
        "returncode": completed.returncode,
        "stderr": completed.stderr[-2000:],
    }


def percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def summarize(samples: list[dict[str, Any]]) -> dict[str, Any]:
    wall = [sample["seconds"] for sample in samples]
    rss = [sample["rss_bytes"] for sample in samples]
    return {
        "runs": len(samples),
        "wall_seconds": {
            "min": min(wall),
            "p50": percentile(wall, 0.50),
            "p95": percentile(wall, 0.95),
            "max": max(wall),
        },
        "peak_rss_bytes": {
            "min": min(rss),
            "p50": percentile([float(value) for value in rss], 0.50),
            "max": max(rss),
        },
    }


def build_binary(binary: Path, skip_build: bool) -> None:
    if skip_build:
        return
    subprocess.run(
        ["cargo", "build", "--locked", "--release", "-p", "mdok-cli"],
        cwd=ROOT,
        check=True,
    )
    if not binary.exists():
        raise RuntimeError(f"release binary was not produced: {binary}")


def run_case(
    binary: Path,
    config: Path,
    document: Path,
    mode: str,
    runs: int,
    warmups: int,
    options: list[str] | None = None,
) -> dict[str, Any]:
    command = [
        str(binary),
        "--config",
        str(config),
        *(options or []),
        "--json",
        mode,
        str(document),
    ]
    for _ in range(warmups):
        sample = measure(command)
        if sample["returncode"] != 0:
            raise RuntimeError(f"warmup failed for {mode}: {sample['stderr']}")
    samples = []
    for _ in range(runs):
        sample = measure(command)
        if sample["returncode"] != 0:
            raise RuntimeError(f"run failed for {mode}: {sample['stderr']}")
        samples.append(sample)
    return summarize(samples)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=DEFAULT_BINARY)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--output", type=Path, default=ROOT / "target" / "performance-bench.json")
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    if args.runs < 1 or args.warmups < 0:
        parser.error("--runs must be positive and --warmups cannot be negative")

    binary = args.binary.resolve()
    build_binary(binary, args.skip_build)
    if not binary.exists():
        parser.error(f"binary does not exist: {binary}")

    results: dict[str, Any] = {
        "platform": platform.platform(),
        "python": platform.python_version(),
        "binary": str(binary),
        "workloads": {},
    }
    with tempfile.TemporaryDirectory(prefix="mdok-performance-") as temporary:
        temp_dir = Path(temporary)
        with FixtureServer() as endpoint:
            config = temp_dir / "mdok.toml"
            config.write_text(
                "[policy]\n"
                "allow_private_network = true\n"
                'allowed_hosts = ["127.0.0.1"]\n',
                encoding="utf-8",
            )
            for label, spec in WORKLOADS.items():
                document = temp_dir / f"{label}.md"
                document.write_text(
                    markdown_source(label, spec["steps"], spec["prose_bytes"], endpoint),
                    encoding="utf-8",
                )
                cases = {}
                for mode in ("lint", "plan", "test"):
                    print(f"{label}/{mode}: measuring {args.runs} runs", flush=True)
                    cases[mode] = run_case(
                        binary, config, document, mode, args.runs, args.warmups
                    )
                results["workloads"][label] = {
                    "document_bytes": document.stat().st_size,
                    "steps": spec["steps"],
                    "cases": cases,
                }
                if label == "intense":
                    discovery_dir = temp_dir / "discovery-1000"
                    discovery_dir.mkdir()
                    discovery_count = 1_000
                    for index in range(discovery_count):
                        (discovery_dir / f"document-{index:04d}.md").write_text(
                            discovery_source(index, endpoint), encoding="utf-8"
                        )
                    cases["plan_1000_docs"] = run_case(
                        binary,
                        config,
                        discovery_dir,
                        "plan",
                        args.runs,
                        args.warmups,
                        options=["--jobs", "4"],
                    )
                    results["workloads"][label]["discovery_documents"] = discovery_count
                    results["workloads"][label]["discovery_jobs"] = 4

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    for label, workload in results["workloads"].items():
        for mode, case in workload["cases"].items():
            wall = case["wall_seconds"]
            rss = case["peak_rss_bytes"]["p50"] / (1024 * 1024)
            print(
                f"{label}/{mode}: p50={wall['p50'] * 1000:.2f} ms "
                f"p95={wall['p95'] * 1000:.2f} ms rss-p50={rss:.2f} MiB"
            )
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
