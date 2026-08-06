#!/usr/bin/env python3
"""Fetch ~100 random Postman collections (with JS scripts) from GitHub.

Contract: docs/QUICKJS_PROBE_SPEC.md section 8. Stdlib + requests only.

Output:
  tests/corpus/postman-js/collections/<index>-<sanitized-name>.json  (verbatim bytes)
  tests/corpus/postman-js/corpus.json                                (manifest)

Usage:
  python3 scripts/fetch_postman_corpus.py [--limit 100] [--seed 0]
                                          [--resume] [--force]
                                          [--workers 8] [--search-pages 10]
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import random
import re
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import requests

# --------------------------------------------------------------------------
# Constants / defaults (mirror the spec)
# --------------------------------------------------------------------------

ROOT = Path(__file__).resolve().parent.parent
CORPUS_DIR = ROOT / "tests" / "corpus" / "postman-js"
COLLECTIONS_DIR = CORPUS_DIR / "collections"
CLONES_DIR = ROOT / "target" / "postman-corpus-clones"

SEARCH_QUERIES = [
    "topic:postman",
    "postman collection in:name",
    "postman-collection in:name",
    "postman api in:name,description",
    "newman in:name",
]
SEARCH_PER_PAGE = 100
SEARCH_MAX_PAGES = 10          # GitHub caps search results at 1000 per query
MAX_BLOB_BYTES = 8 * 1024 * 1024  # skip files > 8 MB
DEFAULT_WORKERS = 8
DEFAULT_LIMIT = 100
DEFAULT_SEED = 0
CLONE_TIMEOUT_S = 300
LS_TREE_TIMEOUT_S = 180
SHOW_TIMEOUT_S = 120
CHUNK_SIZE = 50                # repos per discovery chunk (deterministic)

MANIFEST_VERSION = "1"

_UA = "mdok-postman-corpus-fetcher/1.0"


def log(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def utcnow_iso() -> str:
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


# --------------------------------------------------------------------------
# GitHub search (unauthenticated, rate-limit aware)
# --------------------------------------------------------------------------

class RateLimited(RuntimeError):
    pass


class SearchFetcher:
    """One session used for all search calls; honors the 10 req/min bucket."""

    def __init__(self, session: requests.Session):
        self.session = session
        self._lock = threading.Lock()
        self._min_interval = 1.0   # polite base spacing
        self._last_request = 0.0

    def _pace(self, resp: requests.Response) -> None:
        """Sleep to stay inside the rate-limit bucket (search API)."""
        remaining = resp.headers.get("X-RateLimit-Remaining")
        reset = resp.headers.get("X-RateLimit-Reset")
        try:
            remaining = int(remaining) if remaining is not None else None
        except ValueError:
            remaining = None
        try:
            reset_ts = int(reset) if reset is not None else None
        except ValueError:
            reset_ts = None

        if remaining is not None and remaining <= 0 and reset_ts:
            wait = max(0.0, reset_ts - time.time()) + 2.0
            if wait > 1.0:
                log(f"[search] rate limit exhausted: sleeping {wait:.0f}s until reset")
                time.sleep(wait)
            return

        # otherwise keep a small minimum interval between calls
        elapsed = time.time() - self._last_request
        if elapsed < self._min_interval:
            time.sleep(self._min_interval - elapsed)

    def get(self, url: str, params: dict) -> requests.Response:
        with self._lock:
            for attempt in range(2):
                resp = self.session.get(url, params=params, timeout=60)
                self._last_request = time.time()
                if resp.status_code in (200,):
                    self._pace(resp)
                    return resp
                if resp.status_code in (403, 429):
                    # rate limited (possibly secondary); sleep until reset
                    retry_after = resp.headers.get("Retry-After")
                    reset = resp.headers.get("X-RateLimit-Reset")
                    if reset and reset.isdigit():
                        wait = max(0.0, int(reset) - time.time()) + 2.0
                    elif retry_after and retry_after.isdigit():
                        wait = float(retry_after) + 2.0
                    else:
                        wait = 60.0
                    log(f"[search] HTTP {resp.status_code} (rate limit): "
                        f"sleeping {wait:.0f}s (attempt {attempt + 1}/2)")
                    time.sleep(wait)
                    continue
                if resp.status_code >= 500:
                    time.sleep(5.0)
                    continue
                log(f"[search] HTTP {resp.status_code} for {url} {params}: "
                    f"{resp.text[:200]!r}")
                resp.raise_for_status()
            raise RateLimited(f"search still rate limited after retries: {url}")


def fetch_candidate_repos(seed: int, search_pages: int) -> list[dict]:
    """Collect candidate repos from the spec'd queries, dedupe, shuffle.

    Returns a list of dicts {full_name, html_url} in seeded-shuffled order.
    """
    session = requests.Session()
    session.headers.update({"Accept": "application/vnd.github+json", "User-Agent": _UA})
    fetcher = SearchFetcher(session)

    by_full_name: dict[str, dict] = {}
    for q in SEARCH_QUERIES:
        for page in range(1, search_pages + 1):
            params = {"q": q, "per_page": SEARCH_PER_PAGE, "page": page}
            try:
                resp = fetcher.get("https://api.github.com/search/repositories", params)
            except Exception as exc:  # noqa: BLE001 - keep going
                log(f"[search] query {q!r} page {page} failed: {exc}; continuing")
                break
            try:
                data = resp.json()
            except ValueError:
                log(f"[search] bad JSON for {q!r} page {page}; continuing")
                break
            items = data.get("items") or []
            total = data.get("total_count", 0)
            for it in items:
                full = it.get("full_name") or ""
                if not full:
                    continue
                by_full_name[full.lower()] = {
                    "full_name": full,
                    "html_url": it.get("html_url") or f"https://github.com/{full}",
                }
            log(f"[search] {q!r} page {page}: {len(items)} results "
                f"(total_count={total}, unique repos so far={len(by_full_name)})")
            if len(items) < SEARCH_PER_PAGE:
                break
            if page * SEARCH_PER_PAGE >= min(total or 0, SEARCH_MAX_PAGES * SEARCH_PER_PAGE):
                break

    repos = list(by_full_name.values())
    rng = random.Random(seed)
    rng.shuffle(repos)
    log(f"[search] {len(repos)} unique candidate repos after dedupe+shuffle (seed={seed})")
    return repos


# --------------------------------------------------------------------------
# Git helpers (partial clones under target/postman-corpus-clones/)
# --------------------------------------------------------------------------

def _git(dest: Path, args: list[str], *, timeout: int, check: bool = True,
         capture: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", *args], cwd=str(dest), capture_output=capture, text=False,
        timeout=timeout, check=check,
    )


def clone_repo(repo: dict, log_prefix: str) -> Path | None:
    """Blobless sparse shallow clone; returns clone dir or None on failure.

    Cached clones are reused across runs (target/ is transient/gitignored).
    """
    full_name = repo["full_name"]
    dest = CLONES_DIR / full_name.replace("/", "__")
    if (dest / ".git").exists():
        return dest
    tmp = CLONES_DIR / (full_name.replace("/", "__") + ".tmp")
    if tmp.exists():
        subprocess.run(["rm", "-rf", str(tmp)], check=False)
    tmp.parent.mkdir(parents=True, exist_ok=True)
    url = f"https://github.com/{full_name}.git"
    cmd = ["git", "clone", "--depth", "1", "--filter=blob:none", "--sparse",
           "--no-checkout", url, str(tmp)]
    for attempt in range(2):
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=CLONE_TIMEOUT_S)
        if proc.returncode == 0 and (tmp / ".git").exists():
            # sanity: repo must have a HEAD commit (empty repos have none)
            try:
                _git(tmp, ["rev-parse", "--verify", "HEAD"], timeout=LS_TREE_TIMEOUT_S)
            except (subprocess.SubprocessError, FileNotFoundError):
                pass
            else:
                tmp.rename(dest)
                log(f"{log_prefix} cloned {full_name}")
                return dest
        # transient failure (or empty repo): clean up and retry once
        log(f"{log_prefix} clone {full_name} attempt {attempt + 1} failed "
            f"(rc={proc.returncode}): {(proc.stderr or proc.stdout or '')[:300]!r}")
        subprocess.run(["rm", "-rf", str(tmp)], check=False)
        time.sleep(2.0 + attempt * 3.0)
    return None


def ls_tree_entries(dest: Path, full_name: str = "") -> list[tuple[str, int]]:
    """Return [(path, blob_size)] for every blob in HEAD (no blob fetch).

    Results are cached under target/ keyed by (repo, HEAD sha) so re-runs
    avoid re-walking huge trees.
    """
    head_sha = ""
    if full_name:
        try:
            out = _git(dest, ["rev-parse", "HEAD"], timeout=30)
            head_sha = out.stdout.decode("utf-8", errors="replace").strip()
        except (subprocess.SubprocessError, FileNotFoundError):
            head_sha = ""
        cache = CLONES_DIR / f"lstree-{full_name.replace('/', '__')}-{head_sha[:40]}.json"
        if head_sha and cache.exists():
            try:
                return [tuple(x) for x in json.loads(cache.read_text())]  # type: ignore[misc]
            except (ValueError, OSError):
                pass
    out = _git(dest, ["ls-tree", "-r", "-l", "HEAD"], timeout=LS_TREE_TIMEOUT_S)
    entries: list[tuple[str, int]] = []
    for line in out.stdout.decode("utf-8", errors="surrogateescape").splitlines():
        meta, _, path = line.partition("\t")
        fields = meta.split()
        if len(fields) == 4 and fields[1] == "blob":
            try:
                size = int(fields[3])
            except ValueError:
                continue
            entries.append((path, size))
    if full_name and head_sha:
        try:
            cache.write_text(json.dumps(entries))
        except OSError:
            pass
    return entries


def git_show_blob(dest: Path, path: str) -> bytes | None:
    """Fetch blob at HEAD:<path> (on demand for partial clones); retry once."""
    for attempt in range(2):
        try:
            out = _git(dest, ["show", f"HEAD:{path}"], timeout=SHOW_TIMEOUT_S)
        except (subprocess.SubprocessError, FileNotFoundError) as exc:
            log(f"    git show {path} failed (attempt {attempt + 1}): {exc}")
        else:
            if out.returncode == 0:
                return out.stdout
            log(f"    git show {path} rc={out.returncode} (attempt {attempt + 1})")
        time.sleep(1.0 + attempt)
    return None


# --------------------------------------------------------------------------
# Collection analysis
# --------------------------------------------------------------------------

def join_exec(exec_val) -> str:
    if isinstance(exec_val, str):
        return exec_val
    if isinstance(exec_val, list):
        return "\n".join(str(x) for x in exec_val if x is not None)
    return ""


def looks_like_collection(data) -> bool:
    if not isinstance(data, dict):
        return False
    info = data.get("info")
    if not isinstance(info, dict):
        return False
    schema = info.get("schema")
    if not isinstance(schema, str) or "collection/v2.1" not in schema:
        return False
    return isinstance(data.get("item"), list)


def walk_events(node) -> list[dict]:
    """Recursively collect event dicts from collection/folder/request nodes."""
    events: list[dict] = []
    if not isinstance(node, dict):
        return events
    evs = node.get("event")
    if isinstance(evs, list):
        events.extend(e for e in evs if isinstance(e, dict))
    items = node.get("item")
    if isinstance(items, list):
        for child in items:
            events.extend(walk_events(child))
    return events


def analyze_collection(data: dict) -> dict | None:
    """Return stats dict or None if not a v2.1 collection with >=1 pm. script."""
    if not looks_like_collection(data):
        return None
    scripts: list[tuple[str, str]] = []  # (listen, source)
    for ev in walk_events(data):
        listen = ev.get("listen")
        if listen not in ("test", "prerequest"):
            continue
        script = ev.get("script")
        if not isinstance(script, dict):
            continue
        source = join_exec(script.get("exec"))
        if not source.strip():
            continue
        if "pm." not in source:
            continue
        scripts.append((listen, source))

    if not scripts:
        return None

    info = data.get("info") or {}
    name = info.get("name") or ""
    if not isinstance(name, str) or not name.strip():
        name = "collection"

    test_n = sum(1 for l, _ in scripts if l == "test")
    pre_n = sum(1 for l, _ in scripts if l == "prerequest")
    return {
        "name": name,
        "script_count": len(scripts),
        "script_events": {"test": test_n, "prerequest": pre_n},
        "js_scripts": [hashlib.sha256(s.encode("utf-8")).hexdigest() for _, s in scripts],
    }


def sanitize_name(name: str) -> str:
    s = re.sub(r"[^A-Za-z0-9]+", "-", name).strip("-").lower()
    s = re.sub(r"-+", "-", s)
    s = s[:80].rstrip("-")
    return s or "collection"


def default_branch(dest: Path) -> str:
    """Resolve the clone's default branch (origin/HEAD), fallback 'HEAD'."""
    try:
        out = _git(dest, ["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
                   timeout=30)
        branch = out.stdout.decode("utf-8", errors="replace").strip()
        if branch.startswith("origin/"):
            branch = branch[len("origin/"):]
        if branch and "/" not in branch:
            return branch
    except (subprocess.SubprocessError, FileNotFoundError):
        pass
    return "HEAD"


def collection_source_url(repo: dict, path: str) -> str:
    branch = repo.get("_branch", "HEAD")
    return f"https://raw.githubusercontent.com/{repo['full_name']}/{branch}/{path}"


# --------------------------------------------------------------------------
# Per-repo discovery
# --------------------------------------------------------------------------

def discover_repo(repo: dict, repo_index: int, repo_total: int) -> tuple[str, list]:
    """Clone one repo, find qualifying collections.

    Returns (full_name, [(path, stats), ...]) with paths sorted (deterministic).
    """
    full_name = repo["full_name"]
    prefix = f"[repo {repo_index}/{repo_total} {full_name}]"
    dest = clone_repo(repo, prefix)
    if dest is None:
        log(f"{prefix} skipped (clone failed)")
        return full_name, []
    repo.setdefault("_branch", default_branch(dest))

    try:
        entries = ls_tree_entries(dest, full_name)
    except (subprocess.SubprocessError, FileNotFoundError) as exc:
        log(f"{prefix} ls-tree failed: {exc}; skipped")
        return full_name, []

    candidates: list[str] = []
    generic_json: list[tuple[str, int]] = []
    for path, size in entries:
        if size > MAX_BLOB_BYTES:
            continue
        low = path.lower()
        if low.endswith(".postman_collection.json") or low.endswith(".postman_collection"):
            candidates.append(path)
        elif low.startswith("postman/") and low.endswith(".json"):
            candidates.append(path)
        elif low.endswith(".json"):
            generic_json.append((path, size))

    # content-check generic *.json (bounded, sorted, deterministic);
    # skip obvious noise dirs so a single monorepo cannot stall a chunk
    NOISE_DIRS = ("/node_modules/", "/dist/", "/build/", "/vendor/", "/coverage/", "/.git/", "/assets/")
    generic_json = [(pa, sz) for pa, sz in generic_json
                    if not any(nd in ("/" + pa.lower()) for nd in NOISE_DIRS)]
    generic_json.sort()
    for path, _size in generic_json[:150]:
        blob = git_show_blob(dest, path)
        if blob is None:
            continue
        try:
            data = json.loads(blob.decode("utf-8", errors="replace"))
        except ValueError:
            continue
        if looks_like_collection(data):
            candidates.append(path)

    qualified: list[tuple[str, dict]] = []
    for path in sorted(set(candidates)):
        blob = git_show_blob(dest, path)
        if blob is None:
            continue
        try:
            data = json.loads(blob.decode("utf-8", errors="replace"))
        except ValueError:
            continue
        stats = analyze_collection(data)
        if stats is not None:
            qualified.append((path, stats))

    log(f"{prefix} {len(qualified)} qualifying collection(s)")
    return full_name, qualified


# --------------------------------------------------------------------------
# Main pipeline
# --------------------------------------------------------------------------

def build_ordered_candidates(repos: list[dict], limit: int, workers: int
                             ) -> list[tuple[dict, str, dict]]:
    """Discover candidates chunk-by-chunk until `limit` qualified are found.

    Returns [(repo, path, stats)] in deterministic order (repo shuffled order,
    then sorted path), truncated to `limit`.
    """
    ordered: list[tuple[dict, str, dict]] = []
    total_processed = 0
    for start in range(0, len(repos), CHUNK_SIZE):
        chunk = repos[start:start + CHUNK_SIZE]
        results: list[tuple[str, list]] = [None] * len(chunk)  # type: ignore[list-item]
        with ThreadPoolExecutor(max_workers=workers) as pool:
            futures = {
                pool.submit(discover_repo, repo, total_processed + i + 1, len(repos)): i
                for i, repo in enumerate(chunk)
            }
            for fut in as_completed(futures):
                i = futures[fut]
                try:
                    results[i] = fut.result()
                except Exception as exc:  # noqa: BLE001 - one bad repo never aborts
                    log(f"[repo] worker crashed: {exc}; continuing")
                    results[i] = ("", [])
        total_processed += len(chunk)
        for full_name, qualified in results:
            if not full_name:
                continue
            repo = next(r for r in chunk if r["full_name"] == full_name)
            for path, stats in qualified:
                ordered.append((repo, path, stats))
        log(f"[progress] processed {total_processed}/{len(repos)} repos, "
            f"{len(ordered)} qualifying collections so far (target {limit})")
        if len(ordered) >= limit:
            break
        if total_processed >= len(repos):
            break
    return ordered[:limit]


def sha256_bytes(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--limit", type=int, default=DEFAULT_LIMIT,
                    help=f"stop after this many collections (default {DEFAULT_LIMIT})")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED,
                    help=f"RNG seed for repo shuffle (default {DEFAULT_SEED})")
    ap.add_argument("--workers", type=int, default=DEFAULT_WORKERS,
                    help=f"parallel clone/discover workers (default {DEFAULT_WORKERS})")
    ap.add_argument("--search-pages", type=int, default=SEARCH_MAX_PAGES,
                    help=f"max search result pages per query (default {SEARCH_MAX_PAGES})")
    ap.add_argument("--resume", action="store_true",
                    help="skip entries already present in the manifest")
    ap.add_argument("--force", action="store_true",
                    help="re-download everything, rebuilding the manifest")
    args = ap.parse_args()

    if args.limit <= 0:
        log("--limit must be > 0")
        return 2
    if args.workers <= 0:
        log("--workers must be > 0")
        return 2

    started = time.time()
    COLLECTIONS_DIR.mkdir(parents=True, exist_ok=True)
    CLONES_DIR.mkdir(parents=True, exist_ok=True)
    t_phase = {"search": 0.0, "discover": 0.0, "download": 0.0}

    # ---- existing manifest (for --resume) ---------------------------------
    existing: dict[tuple[str, str], dict] = {}
    manifest_path = CORPUS_DIR / "corpus.json"
    if manifest_path.exists() and not args.force:
        try:
            old = json.loads(manifest_path.read_text())
            for e in old.get("entries", []):
                existing.setdefault((e.get("source_url"), e.get("sha256")), e)
            log(f"[resume] loaded {len(existing)} existing entries from corpus.json")
        except Exception as exc:  # noqa: BLE001
            log(f"[resume] could not parse existing corpus.json ({exc}); starting fresh")

    # ---- search -----------------------------------------------------------
    _t0 = time.time()
    cache_path = CLONES_DIR / f"search-cache-seed{args.seed}-pages{args.search_pages}.json"
    if not args.force and cache_path.exists():
        repos = json.loads(cache_path.read_text())
        log(f"[search] loaded {len(repos)} candidate repos from cache {cache_path}")
    else:
        repos = fetch_candidate_repos(args.seed, args.search_pages)
        cache_path.write_text(json.dumps(repos))
        log(f"[search] wrote {len(repos)} candidate repos to cache {cache_path}")
    t_phase["search"] = time.time() - _t0
    if not repos:
        log("no candidate repos found; aborting")
        return 1

    # ---- discover candidates (deterministic order) ------------------------
    _t0 = time.time()
    log(f"[discover] scanning repos for qualifying collections (limit={args.limit})")
    ordered = build_ordered_candidates(repos, args.limit, args.workers)
    t_phase["discover"] = time.time() - _t0
    log(f"[discover] {len(ordered)} qualifying collections selected (ordered)")

    if not ordered:
        log("no qualifying collections found; aborting")
        return 1

    # ---- assign indices & download ----------------------------------------
    entries: list[dict] = []
    existing_indices = {e["index"] for e in existing.values()}
    next_index = max(existing_indices, default=-1) + 1
    seen_sources: dict[tuple[str, str], dict] = {  # resume map
        (e.get("source_url"), e.get("sha256")): e for e in existing.values()
    }

    to_download: list[tuple[dict, str, dict, str]] = []
    for repo, path, stats in ordered:
        source_url = collection_source_url(repo, path)
        # sha256 unknown until fetch; resume-matching happens after
        to_download.append((repo, path, stats, source_url))

    # ---- fetch blobs (parallel), then write in deterministic order --------
    _t0 = time.time()

    def download_one(item) -> dict:
        repo, path, stats, source_url = item
        dest = CLONES_DIR / repo["full_name"].replace("/", "__")
        blob = git_show_blob(dest, path)
        if blob is None:
            return {"_failed": True, "source_url": source_url, "path": path}
        return {
            "repo": repo, "path": path, "stats": stats, "source_url": source_url,
            "blob": blob, "sha256": sha256_bytes(blob), "byte_size": len(blob),
        }

    fetched: list[dict] = [None] * len(to_download)  # type: ignore[list-item]
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futs = {pool.submit(download_one, item): i for i, item in enumerate(to_download)}
        for fut in as_completed(futs):
            i = futs[fut]
            try:
                fetched[i] = fut.result()
            except Exception as exc:  # noqa: BLE001
                log(f"[download] worker {i} crashed: {exc}")
                fetched[i] = {"_failed": True}

    # deterministic write pass
    entries = []
    for slot, item in enumerate(to_download):
        res = fetched[slot]
        if res is None or res.get("_failed"):
            log(f"[download] failed to fetch {item[3]}; skipped")
            continue
        repo, path, stats, source_url = res["repo"], res["path"], res["stats"], res["source_url"]
        blob = res["blob"]

        if args.resume:
            key = (source_url, res["sha256"])
            old_entry = seen_sources.get(key)
            if old_entry is not None:
                old_fname = (f"{old_entry['index']:04d}-"
                             f"{sanitize_name(old_entry['name'])}.json")
                if (COLLECTIONS_DIR / old_fname).exists():
                    # keep existing entry, don't rewrite
                    entries.append(old_entry)
                    log(f"[download] {old_entry['index']:04d} resume: kept {source_url}")
                    continue

        index = next_index
        next_index += 1
        fname = f"{index:04d}-{sanitize_name(stats['name'])}.json"
        out_path = COLLECTIONS_DIR / fname
        tmp_path = out_path.with_suffix(out_path.suffix + ".tmp")
        tmp_path.write_bytes(blob)
        os.replace(tmp_path, out_path)

        entry = {
            "index": index,
            "name": stats["name"],
            "source_url": source_url,
            "sha256": res["sha256"],
            "byte_size": res["byte_size"],
            "script_count": stats["script_count"],
            "script_events": stats["script_events"],
            "js_scripts": stats["js_scripts"],
        }
        entries.append(entry)
        log(f"[download] {index:04d}/{args.limit} saved {fname} "
            f"({res['byte_size']} B, {stats['script_count']} scripts) from {source_url}")

    entries.sort(key=lambda e: e["index"])
    # if limit was reached with fresh indices but resume kept some, trim to limit
    entries = entries[:args.limit]

    t_phase["download"] = time.time() - _t0

    # ---- write manifest ----------------------------------------------------
    manifest = {
        "version": MANIFEST_VERSION,
        "seed": args.seed,
        "fetched_at": utcnow_iso(),
        "entries": entries,
    }
    tmp_manifest = manifest_path.with_suffix(manifest_path.suffix + ".tmp")
    tmp_manifest.write_text(json.dumps(manifest, indent=2) + "\n")
    os.replace(tmp_manifest, manifest_path)

    # remove orphaned collection files (anything not exactly referenced by
    # the manifest: stale indices or stale names from an earlier run)
    expected = {
        f"{e['index']:04d}-{sanitize_name(e['name'])}.json" for e in entries
    }
    removed = 0
    for f in sorted(COLLECTIONS_DIR.glob("*.json")):
        if f.name not in expected:
            f.unlink()
            removed += 1
    if removed:
        log(f"[cleanup] removed {removed} orphaned collection file(s)")

    elapsed = time.time() - started

    # ---- summary -----------------------------------------------------------
    total_bytes = sum(e["byte_size"] for e in entries)
    repo_hist: dict[str, int] = {}
    for e in entries:
        m = re.match(r"https://raw\.githubusercontent\.com/([^/]+/[^/]+)/", e["source_url"])
        repo = m.group(1) if m else "unknown"
        repo_hist[repo] = repo_hist.get(repo, 0) + 1
    test_n = sum(e["script_events"]["test"] for e in entries)
    pre_n = sum(e["script_events"]["prerequest"] for e in entries)

    log("")
    log("=" * 70)
    log("Corpus summary")
    log("=" * 70)
    log(f"collections : {len(entries)} (target {args.limit})")
    log(f"total bytes : {total_bytes:,}")
    log(f"seed        : {args.seed}")
    log(f"elapsed     : {elapsed:.1f}s "
        f"(search {t_phase['search']:.0f}s, discover {t_phase['discover']:.0f}s, "
        f"download {t_phase['download']:.0f}s)")
    log(f"script events: test={test_n} prerequest={pre_n} (total {test_n + pre_n})")
    log("source repos (top 15):")
    for repo, n in sorted(repo_hist.items(), key=lambda kv: -kv[1])[:15]:
        log(f"  {repo}: {n}")
    log("script-event histogram (per collection):")
    from collections import Counter
    dist = Counter((e["script_events"]["test"], e["script_events"]["prerequest"]) for e in entries)
    for (t, p), n in sorted(dist.items(), key=lambda kv: (-kv[0][0], -kv[0][1])):
        log(f"  test={t:3d} prerequest={p:3d}: {n} collection(s)")
    log(f"manifest    : {manifest_path}")
    log("=" * 70)

    return 0


if __name__ == "__main__":
    sys.exit(main())
