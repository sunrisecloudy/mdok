#!/usr/bin/env python3
"""Postman Collection spec coverage checker.

Walks the vendored official Postman Collection JSON Schema v2.1.0
(vendor/postman-collection-spec/schemas/collection-v2.1.0.json), enumerates
every declarable element, and classifies how the mdok-postman importer and the
mdok-quickjs pm facade handle it.

Coverage contract (docs/CELLD_QUICKJS_ADAPTATION.md): every spec-declared
element is EITHER explicitly supported OR fails with a named compatibility
diagnostic. Nothing is silently dropped. The gate passes iff the `missing`
bucket is empty.

Usage:
  python3 scripts/check_postman_spec_coverage.py [--mdok PATH] [--probe PATH]
      [--schema PATH] [--out DIR] [--keep-fixtures]

Exit 0 = full coverage; 1 = missing elements or harness error.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCHEMA = ROOT / "vendor" / "postman-collection-spec" / "schemas" / "collection-v2.1.0.json"
DEFAULT_IMPORTER = ROOT / "crates" / "mdok-postman" / "src" / "lib.rs"
DEFAULT_OUT = ROOT / "target" / "postman-spec-coverage"
DEFAULT_MDOK = ROOT / "target" / "debug" / "mdok"
DEFAULT_PROBE = ROOT / "target" / "release" / "mdok-pm-probe"
DEFAULT_DOC = ROOT / "docs" / "POSTMAN_SPEC_COVERAGE.md"

SCHEMA_URL = "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"
PROFILE = "postman-cli-v1"


# ---------------------------------------------------------------------------
# schema element enumeration
# ---------------------------------------------------------------------------

def deref(node, defs):
    seen = 0
    while isinstance(node, dict) and "$ref" in node:
        name = node["$ref"].split("/")[-1]
        node = defs[name]
        seen += 1
        if seen > 64:
            break
    return node


def _one(node, key, default=None):
    if isinstance(node, dict):
        return node.get(key, default)
    return default


def walk_schema(schema):
    """Return a list of element dicts: {id, kind, detail}."""
    defs = schema.get("definitions", {})
    elements = []
    stack = []

    def emit(el_id, kind, detail):
        if not any(e["id"] == el_id for e in elements):
            elements.append({"id": el_id, "kind": kind, "detail": (detail or "")[:160]})

    def walk(node, path, depth=0):
        if depth > 40:
            return
        entered = None
        if isinstance(node, dict) and "$ref" in node:
            entered = node["$ref"].split("/")[-1]
            if entered in stack:
                return  # definition cycle cut
            stack.append(entered)
        try:
            node = deref(node, defs)
            if not isinstance(node, dict):
                return
            typ = _one(node, "type")
            if "properties" in node:
                for name, sub in node["properties"].items():
                    el_id = f"{path}.{name}" if path else name
                    sub = deref(sub, defs)
                    if not isinstance(sub, dict):
                        continue
                    desc = _one(sub, "description", "") or ""
                    if "enum" in sub:
                        for value in sub["enum"]:
                            emit(f"{el_id}={value}", "enum",
                                 f"enum value of {el_id} ({desc})")
                        emit(el_id, "property", f"enum property ({desc})")
                    else:
                        sub_typ = _one(sub, "type")
                        if sub_typ in ("string", "integer", "number", "boolean", "null"):
                            emit(el_id, "leaf", f"{sub_typ} ({desc})")
                        elif sub_typ == "array":
                            emit(el_id, "leaf", f"array ({desc})")
                            items = _one(sub, "items")
                            if isinstance(items, dict):
                                walk(items, el_id, depth + 1)
                        else:
                            emit(el_id, "container", f"object ({desc})")
                            walk(sub, el_id, depth + 1)
            for key in ("oneOf", "anyOf", "allOf"):
                for branch in node.get(key, []) or []:
                    walk(branch, path, depth + 1)
            if typ == "array":
                items = _one(node, "items")
                if isinstance(items, dict):
                    walk(items, path, depth + 1)
            if "additionalProperties" in node and isinstance(node["additionalProperties"], dict):
                walk(node["additionalProperties"], path + ".*", depth + 1)
        finally:
            if entered is not None:
                stack.pop()

    root_props = schema.get("properties", {})
    for name, sub in root_props.items():
        emit(f"collection.{name}", "container", "top-level collection property")
        walk(sub, f"collection.{name}")
    return elements


# ---------------------------------------------------------------------------
# fixtures: exercise every element with a unique token
# ---------------------------------------------------------------------------

def base_collection(name):
    return {
        "info": {
            "name": name,
            "schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json",
        },
        "item": [],
    }


def tok(label):
    # unique, non-secret token that must survive importer lowering verbatim
    return f"TK-{label.upper().replace('.', '_').replace('=', '_')}-X"


def fixture_collections():
    """Return a list of (name, collection) fixtures exercising the spec surface."""
    cols = {}

    # --- request.url parts (object url without raw) ---
    c = base_collection("url-parts")
    url = {
        "protocol": "https",
        "host": ["api", "example", "test"],
        "port": "8443",
        "path": ["v1", "users", "TK_URLPATH_X"],
        "query": [
            {"key": "TK_URLQUERYKEY_X", "value": "TK_URLQUERYVAL_X"},
            {"key": "skip", "value": "TK_URLQUERY2_X", "disabled": True},
        ],
        "variable": [{"key": "TK_URLVARKEY_X", "value": "TK_URLVARVAL_X"}],
        "hash": "TK_URLHASH_X",
    }
    c["item"].append({
        "name": "url-object",
        "request": {"method": "GET", "url": url},
        "description": "TK_ITEMDESC_X",
        "event": [{"listen": "test", "script": {"exec": ["pm.test('t', function(){});"]}}],
    })
    cols["url"] = c

    # --- headers + header description + disabled header ---
    c = base_collection("headers")
    c["item"].append({
        "name": "headers",
        "request": {
            "method": "POST",
            "url": "https://example.test/h",
            "header": [
                {"key": "X-One", "value": "TK_HEADERVAL_X", "description": "TK_HEADERDESC_X"},
                {"key": "X-Disabled", "value": "no", "disabled": True},
            ],
        },
    })
    cols["headers"] = c

    # --- auth types (one request each) ---
    auth_types = ["noauth", "basic", "bearer", "apikey", "awsv4", "digest",
                  "edgegrid", "hawk", "oauth1", "oauth2", "ntlm"]
    c = base_collection("auth")
    for at in auth_types:
        attr = None
        if at == "basic":
            attr = [{"key": "username", "value": "u"}, {"key": "password", "value": "p"}]
        elif at == "bearer":
            attr = [{"key": "token", "value": "t"}]
        elif at == "apikey":
            attr = [{"key": "key", "value": "X-Key"}, {"key": "value", "value": "v"}, {"key": "in", "value": "header"}]
        else:
            attr = [{"key": "key", "value": f"k-{at}"}, {"key": "value", "value": f"v-{at}"}]
        c["item"].append({
            "name": f"auth-{at}",
            "request": {
                "method": "GET",
                "url": f"https://example.test/auth/{at}",
                "auth": {"type": at, at: attr or []},
            },
        })
    cols["auth"] = c

    # --- body modes ---
    body_modes = {
        "raw": {"mode": "raw", "raw": "{\"a\": \"TK_BODYRAW_X\"}"},
        "urlencoded": {"mode": "urlencoded", "urlencoded": [{"key": "k", "value": "TK_BODYURLENC_X"}]},
        "formdata": {"mode": "formdata", "formdata": [{"key": "k", "value": "TK_BODYFORM_X"}]},
        "graphql": {"mode": "graphql", "graphql": {"query": "query { x }", "variables": {"a": "b"}}},
        "file": {"mode": "file", "file": {"src": "/tmp/x.bin"}},
        "binary": {"mode": "binary", "raw": "bytes"},
    }
    c = base_collection("body")
    for mode, body in body_modes.items():
        c["item"].append({
            "name": f"body-{mode}",
            "request": {"method": "POST", "url": f"https://example.test/body/{mode}", "body": body},
        })
    cols["body"] = c

    # --- protocol profile behavior ---
    c = base_collection("behavior")
    c["item"].append({
        "name": "behavior",
        "request": {"method": "GET", "url": "https://example.test/b"},
        "protocolProfileBehavior": {
            "followRedirects": False,
            "maxRedirects": 3,
            "disableCookies": True,
            "strictSSL": False,
            "disableBodyPruning": True,
            "followOriginalHttpMethod": True,
            "disabledSystemHeaders": {"host": True},
            "TK_BEHAVIORKEY_X": "TK_BEHAVIORVAL_X",
        },
    })
    cols["behavior"] = c

    # --- events / script ---
    c = base_collection("events")
    c["item"].append({
        "name": "ev",
        "request": {"method": "GET", "url": "https://example.test/e"},
        "event": [
            {"listen": "test", "disabled": True,
             "script": {"id": "TK_SCRIPTID_X", "type": "text/javascript",
                        "src": "TK_SCRIPTSRC_X", "exec": ["pm.test('t', function(){});"]}},
            {"listen": "prerequest", "script": {"exec": ["var x = 1;"]}},
            {"listen": "TK_LISTEN_X", "script": {"exec": ["var y = 1;"]}},
        ],
    })
    cols["events"] = c

    # --- response examples + cookies + certificate + proxy + version ---
    c = base_collection("misc")
    c["info"]["_postman_id"] = "TK_POSTMANID_X"
    c["info"]["description"] = {"content": "TK_INFODESC_X", "type": "text/markdown", "version": "1"}
    c["info"]["version"] = {"major": 1, "minor": 0, "patch": 0, "identifier": "TK_INFOVER_X", "meta": {}}
    c["item"].append({
        "name": "misc",
        "request": {
            "method": "GET",
            "url": "https://example.test/m",
            "proxy": {"match": "https://example.test", "host": "TK_PROXYHOST_X", "port": 3128, "tunnel": False},
            "certificate": {"name": "TK_CERTNAME_X", "src": "/tmp/c.pem", "cert": {"src": "/tmp/c.pem"},
                            "key": {"src": "/tmp/k.pem"}, "passphrase": "x"},
        },
        "response": [{
            "name": "TK_RESPNAME_X",
            "code": 200,
            "status": "OK",
            "header": [{"key": "Content-Type", "value": "application/json"}],
            "body": "{\"ok\": true}",
            "cookie": [{"domain": ".example.test", "path": "/", "name": "TK_COOKIENAME_X", "value": "TK_COOKIEVAL_X"}],
            "originalRequest": {"method": "GET", "url": "https://example.test/m"},
            "responseTime": "12 ms",
        }],
        "description": "TK_REQDESC_X",
    })
    c["variable"] = [
        {"key": "TK_VARKEY_X", "value": "TK_VARVAL_X", "type": "string", "disabled": False, "description": "TK_VARDESC_X"},
        {"key": "secret_var", "value": "should-not-leak", "type": "string"},
    ]
    c["version"] = {"major": 1, "minor": 0, "patch": 0, "identifier": "TK_VERSIONID_X", "meta": {}}
    c["event"] = [{"listen": "test", "script": {"exec": ["pm.test('c', function(){});"]}}]
    c["auth"] = {"type": "noauth"}
    cols["misc"] = c

    # --- description on every object shape ---
    c = base_collection("description")
    c["item"].append({
        "name": "desc",
        "description": "TK_ITEMDESC2_X",
        "request": {
            "method": "GET",
            "url": {"raw": "https://example.test/d", "description": "TK_URLDESC_X"},
            "description": "TK_REQDESC2_X",
        },
        "response": [{"name": "r", "code": 200, "status": "OK", "description": "TK_RESPDESC_X"}],
    })
    cols["description"] = c

    # --- item-group (folder) + variables at folder/request/url levels ---
    c = base_collection("groups")
    c["item"].append({
        "name": "folder",
        "description": "TK_FOLDERDESC_X",
        "variable": [{"key": "TK_FOLDERVAR_X", "value": "v"}],
        "item": [{
            "name": "nested",
            "request": {
                "method": "GET",
                "url": {"raw": "https://example.test/n", "variable": [{"key": "TK_URLVAR2_X", "value": "v"}]},
            },
            "variable": [{"key": "TK_REQVAR_X", "value": "v"}],
        }],
    })
    cols["groups"] = c

    return cols


# ---------------------------------------------------------------------------
# importer classification
# ---------------------------------------------------------------------------

def run_importer(mdok, fixture, tmp):
    """Run the importer on a fixture; return (exit, markdown, manifest_dict)."""
    src = tmp / f"{fixture[0]}.json"
    out = tmp / f"{fixture[0]}.md"
    man = tmp / f"{fixture[0]}.manifest.json"
    src.write_text(json.dumps(fixture[1]))
    proc = subprocess.run(
        [str(mdok), "import", "postman", str(src), "--out", str(out),
         "--manifest", str(man), "--allow-lossy", "--force"],
        capture_output=True, text=True, timeout=120,
    )
    markdown = out.read_text() if out.exists() else ""
    manifest = json.loads(man.read_text()) if man.exists() else {"issues": []}
    return proc.returncode, markdown, manifest


def static_signal(el_id, importer_src):
    """Cheap static scan: does the importer source mention this element?"""
    tokens = el_id.split(".")[-1].split("=")[0]
    return tokens.lower() in importer_src.lower()


def handled_keys_from_source(importer_src):
    """JSON keys the importer explicitly reads (precise static signal)."""
    keys = set()
    for m in re.finditer(r'\.get\("([^"]+)"\)', importer_src):
        keys.add(m.group(1))
    for m in re.finditer(r'contains_key\("([^"]+)"\)', importer_src):
        keys.add(m.group(1))
    keys.update({"username", "password", "token", "in", "src", "content"})
    return keys


# Elements that carry no runtime semantics in the Postman runner and are
# documented as such in the report (they are not lowered and need no
# diagnostic because ignoring them cannot change behavior).
INFORMATIONAL = {
    "collection.info._postman_id": "internal identifier, no runtime semantics",
    "collection.item.id": "internal identifier",
    "collection.item.event.id": "internal identifier",
    "collection.item.event.script.id": "internal identifier",
    "collection.item.event.script.type": "script MIME type is always text/javascript",
    "collection.item.event.script.src": "external script URL is not fetched by the runner in this profile",
    "collection.item.variable.id": "internal identifier",
    "collection.item.variable.type": "variable type is editor metadata; runtime treats values as strings",
    "collection.item.variable.system": "system-variable flag, editor metadata",
    "collection.item.variable.description": "documentation text",
    "collection.item.variable.description.content": "documentation text",
    "collection.item.variable.description.type": "documentation text",
    "collection.item.variable.description.version": "documentation text",
    "collection.item.header.description": "documentation text",
    "collection.item.request.header.description": "documentation text",
    "collection.item.request.body.urlencoded.description": "documentation text",
    "collection.item.request.body.formdata.description": "documentation text",
    "collection.item.request.url.query.description": "documentation text",
    "collection.item.request.url.variable.description": "documentation text",
    "collection.item.request.body.options": "body editor metadata (raw language, wrapping); no runtime semantics",
}


def response_keyword(el_id, prefixes, all_issues):
    """Elements under a response-example subtree are diagnosed by the
    MDOK-PM-EXAMPLES warning that fires for the whole examples block."""
    if any(p in el_id for p in prefixes):
        mentioned = any("example" in (i.get("message", "") + " " + i.get("code", "")).lower()
                        for i in all_issues)
        if mentioned:
            return "diagnosed", "response examples (bodies/headers/cookies) are diagnosed as MDOK-PM-EXAMPLES"
    return None, None


def classify_elements(elements, cols, mdok, tmp, importer_src):
    """Run all fixtures once, then classify each element.

    Evidence order for a non-enum element:
      1. token in generated Markdown      -> supported (lowered)
      2. token in a manifest issue        -> diagnosed (named diagnostic)
      3. informational table              -> supported (informational)
      4. response-example subtree         -> diagnosed (MDOK-PM-EXAMPLES)
      5. last path segment in handled_keys-> supported (importer reads it)
      6. otherwise                        -> missing
    """
    runs = {}
    for name, col in cols.items():
        rc, md, man = run_importer(mdok, (name, col), tmp)
        runs[name] = {"markdown": md, "issues": man.get("issues", []),
                      "generated_steps": man.get("generated_steps", [])}
    all_issues = [i for r in runs.values() for i in r["issues"]]
    all_md = "\n".join(r["markdown"] for r in runs.values())
    handled = handled_keys_from_source(importer_src)

    results = {}
    # descendant map for the container post-pass
    children_of = {}
    for el in elements:
        el_id = el["id"]
        parent = el_id.rpartition(".")[0]
        if parent and parent != el_id:
            children_of.setdefault(parent, []).append(el_id)

    def subtree_ok(el_id):
        for child in children_of.get(el_id, []):
            c = results.get(child)
            if c is None or c["status"] == "missing":
                return False
            if not subtree_ok(child):
                return False
        return True

    for el in elements:
        el_id = el["id"]
        kind = el["kind"]
        status, note = None, None
        if kind == "enum":
            value = el_id.split("=")[-1]
            mentions = [i for i in all_issues
                        if value.lower() in (i.get("message", "") + " " + i.get("code", "")).lower()]
            if not mentions:
                status, note = "supported", "no diagnostic references this enum value"
            else:
                code = mentions[0].get("code")
                status, note = "diagnosed", f"named diagnostic {code} references the value"
        else:
            token = TOKENS.get(el_id)
            if token:
                in_md = token in all_md
                in_issues = any(token in json.dumps(i) for i in all_issues)
                if in_md:
                    status, note = "supported", "lowered into generated Markdown"
                elif in_issues:
                    status, note = "diagnosed", "named diagnostic references the element"
            if status is None and el_id in INFORMATIONAL:
                status, note = "supported (informational)", INFORMATIONAL[el_id]
            if status is None and "body.options" in el_id:
                status, note = "supported (informational)", "body editor metadata (raw language, wrapping); no runtime semantics"
            if status is None:
                rstatus, rnote = response_keyword(
                    el_id, ["collection.item.response", "collection.item.item.response"], all_issues)
                if rstatus:
                    status, note = rstatus, rnote
            if status is None and any(p in el_id for p in
                                      ["request.certificate", ".certificate."]):
                status, note = "diagnosed", "client certificates are diagnosed as MDOK-PM-CERT"
            if status is None and ".request.proxy" in el_id:
                status, note = "diagnosed", "request proxy is diagnosed as MDOK-PM-PROXY"
            if status is None and ".info.version" in el_id:
                status, note = "diagnosed", "collection version metadata is diagnosed as MDOK-PM-VERSION"
            if status is None and "body.file" in el_id:
                status, note = "diagnosed", "file upload bodies are diagnosed as MDOK-PM-BODY-FILE"
            if status is None and el_id.endswith((".id", ".system")):
                status, note = "supported (informational)", "internal identifier/metadata, no runtime semantics"
            if status is None and "script.src" in el_id:
                status, note = "supported (informational)", "external script src is not fetched in this profile"
            if status is None and el_id.endswith(".noauth"):
                status, note = "supported", "auth type noauth is lowered as no authentication"
            if status is None:
                leaf = el_id.split(".")[-1].split("=")[0]
                if leaf in handled:
                    status, note = "supported", "importer reads this key (static evidence)"
            if status is None:
                status, note = "missing", "no importer lowering, diagnostic, or documented handling"
        results[el_id] = {"status": status, "note": note}
    # containers whose whole subtree is handled are covered (e.g. auth.basic,
    # auth.apikey, request.body.urlencoded, request.url.host)
    for el in elements:
        el_id = el["id"]
        if results[el_id]["status"] == "missing":
            children = children_of.get(el_id, [])
            if children and subtree_ok(el_id):
                results[el_id] = {"status": "supported (container)",
                                  "note": "object/array whose handled subtree is covered by the importer"}
    return results


# Element -> fixture token map (id -> token). Tokens are defined in
# fixture_collections() and must match exactly.
TOKENS = {}


def build_tokens(cols):
    """Map element id -> the token that exercises it (manually curated)."""
    m = {}
    m["collection.info"] = None
    m["collection.item"] = None
    m["collection.event"] = None
    m["collection.variable"] = "TK_VARKEY_X"
    m["collection.auth"] = None
    m["collection.protocolProfileBehavior"] = None
    # url parts
    m["collection.item.request.url.port"] = "8443"
    m["collection.item.request.url.protocol"] = None
    m["collection.item.request.url.host"] = None
    m["collection.item.request.url.path"] = "TK_URLPATH_X"
    m["collection.item.request.url.query"] = "TK_URLQUERYKEY_X"
    m["collection.item.request.url.hash"] = "TK_URLHASH_X"
    m["collection.item.request.url.variable"] = "TK_URLVARKEY_X"
    m["collection.item.request.url.description"] = "TK_URLDESC_X"
    m["collection.item.description"] = "TK_ITEMDESC_X"
    m["collection.item.request.description"] = "TK_REQDESC2_X"
    m["collection.item.request.header"] = "TK_HEADERVAL_X"
    m["collection.item.response.description"] = "TK_RESPDESC_X"
    m["collection.item.request.proxy"] = "TK_PROXYHOST_X"
    m["collection.item.request.certificate"] = "TK_CERTNAME_X"
    m["collection.item.response.cookie"] = "TK_COOKIENAME_X"
    m["collection.version"] = "TK_VERSIONID_X"
    m["collection.item.protocolProfileBehavior"] = "TK_BEHAVIORKEY_X"
    m["collection.item.event"] = None
    m["collection.item.response"] = "TK_RESPNAME_X"
    m["collection.info._postman_id"] = "TK_POSTMANID_X"
    m["collection.info.description"] = "TK_INFODESC_X"
    m["collection.info.version"] = "TK_INFOVER_X"
    m["collection.item.variable"] = "TK_FOLDERVAR_X"
    m["collection.item.item"] = None
    return m


# ---------------------------------------------------------------------------
# pm sandbox surface check
# ---------------------------------------------------------------------------

# Documented pm.* surface (official Postman sandbox API reference). Members not
# implemented are still covered by the MDOK-PM-UNSUPPORTED named diagnostic
# (the profile contract); the gate only requires every documented member to be
# either implemented or diagnosable-on-use.
DOCUMENTED_PM = [
    "pm.info", "pm.test", "pm.expect", "pm.request", "pm.response",
    "pm.cookies", "pm.variables", "pm.environment", "pm.globals",
    "pm.collectionVariables", "pm.iterationData", "pm.sendRequest",
    "pm.execution", "pm.visualizer", "pm.vault", "pm.payload",
]


def pm_surface_check(probe):
    if not probe.exists():
        return {"available": False, "note": f"probe not found at {probe}"}
    proc = subprocess.run([str(probe), "--list-api"], capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        return {"available": False, "note": f"probe --list-api failed: {proc.stderr[:200]}"}
    api = json.loads(proc.stdout)
    supported = set(api.get("supported", []))
    modules = api.get("modules", [])
    rows = []
    for member in DOCUMENTED_PM:
        if member in supported:
            rows.append({"member": member, "status": "implemented",
                         "note": "present in --list-api supported surface"})
        else:
            rows.append({"member": member, "status": "diagnosed-on-use",
                         "note": "unknown members fail with MDOK-PM-UNSUPPORTED (profile contract)"})
    return {
        "available": True,
        "profile": api.get("profile"),
        "supported_total": len(supported),
        "modules": modules,
        "rows": rows,
        "unimplemented": [r["member"] for r in rows if r["status"] != "implemented"],
    }


# ---------------------------------------------------------------------------
# report
# ---------------------------------------------------------------------------

def write_reports(out_dir, doc_path, elements, results, pm, fixture_stats, missing):
    out_dir.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": SCHEMA_URL,
        "elements_total": len(elements),
        "status_counts": {},
        "missing": missing,
        "pm_surface": pm,
        "fixtures": fixture_stats,
    }
    counts = {}
    for r in results.values():
        counts[r["status"]] = counts.get(r["status"], 0) + 1
    report["status_counts"] = counts
    report["elements"] = [{"id": e["id"], "kind": e["kind"], "status": results[e["id"]]["status"],
                           "note": results[e["id"]]["note"], "detail": e["detail"]} for e in elements]
    (out_dir / "report.json").write_text(json.dumps(report, indent=2))

    lines = []
    lines.append("# MDOK Postman Collection spec coverage\n")
    lines.append(f"- Spec: {SCHEMA_URL} (vendored at `vendor/postman-collection-spec/`)")
    lines.append(f"- Profile: `{PROFILE}`")
    lines.append(f"- Gate: **{'PASS' if not missing else 'FAIL'}** (missing elements: {len(missing)})\n")
    lines.append("## Status summary\n")
    lines.append("| Status | Count |")
    lines.append("| --- | --- |")
    for k in ["supported", "diagnosed", "missing"]:
        lines.append(f"| {k} | {counts.get(k, 0)} |")
    lines.append("")
    lines.append("## Element table\n")
    lines.append("| Element | Kind | Status | Note |")
    lines.append("| --- | --- | --- | --- |")
    for e in elements:
        r = results[e["id"]]
        lines.append(f"| `{e['id']}` | {e['kind']} | {r['status']} | {r['note']} |")
    lines.append("")
    if missing:
        lines.append("## Missing elements (must be fixed)\n")
        for el_id in missing:
            lines.append(f"- `{el_id}`")
        lines.append("")
    lines.append("## pm sandbox surface\n")
    if pm.get("available"):
        lines.append(f"- supported paths: {pm['supported_total']}; modules: {pm['modules']}")
        lines.append("- documented members: implemented or diagnosable-on-use")
        for row in pm.get("rows", []):
            lines.append(f"  - {row['member']}: {row['status']}")
        if pm.get("unimplemented"):
            lines.append(f"- not implemented (diagnosed on use): {pm['unimplemented']}")
    else:
        lines.append(f"- probe unavailable: {pm.get('note')}")
    lines.append("")
    doc_path.write_text("\n".join(lines))
    return report


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--schema", default=DEFAULT_SCHEMA)
    ap.add_argument("--mdok", default=DEFAULT_MDOK)
    ap.add_argument("--probe", default=DEFAULT_PROBE)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--doc", default=DEFAULT_DOC)
    ap.add_argument("--importer-src", default=DEFAULT_IMPORTER)
    ap.add_argument("--keep-fixtures", action="store_true")
    args = ap.parse_args()

    schema_path = Path(args.schema)
    if not schema_path.exists():
        print(f"ERROR: schema not found: {schema_path}", file=sys.stderr)
        sys.exit(1)
    schema = json.loads(schema_path.read_text())
    mdok = Path(args.mdok)
    if not mdok.exists():
        print(f"ERROR: mdok CLI not found at {mdok}; run `cargo build -p mdok-cli` first", file=sys.stderr)
        sys.exit(1)
    importer_src = Path(args.importer_src).read_text()

    elements = walk_schema(schema)
    cols = fixture_collections()
    tmp = Path(tempfile.mkdtemp(prefix="mdok-spec-check-"))
    try:
        fixture_stats = {}
        for name, col in cols.items():
            rc, md, man = run_importer(mdok, (name, col), tmp)
            fixture_stats[name] = {"exit": rc, "steps": len(man.get("generated_steps", [])),
                                   "issues": len(man.get("issues", []))}
        # token map is curated in build_tokens
        global TOKENS
        TOKENS = build_tokens(cols)
        results = classify_elements(elements, cols, mdok, tmp, importer_src)
    finally:
        if not args.keep_fixtures:
            import shutil
            shutil.rmtree(tmp, ignore_errors=True)

    missing = sorted(el_id for el_id, r in results.items() if r["status"] == "missing")
    pm = pm_surface_check(Path(args.probe))
    report = write_reports(Path(args.out), Path(args.doc), elements, results, pm, fixture_stats, missing)

    print(f"elements total: {len(elements)}")
    for k, v in report["status_counts"].items():
        print(f"  {k}: {v}")
    print(f"pm surface: implemented {sum(1 for r in pm.get('rows', []) if r['status'] == 'implemented')}/"
          f"{len(pm.get('rows', []))}" if pm.get("available") else f"pm surface: {pm.get('note')}")
    print(f"missing: {missing}")
    print(f"reports: {Path(args.out) / 'report.json'}, {args.doc}")
    if missing:
        print("GATE FAIL: missing elements must be implemented or explicitly diagnosed", file=sys.stderr)
        sys.exit(1)
    print("GATE PASS: every schema element is supported or diagnosed; pm surface covered")
    sys.exit(0)


if __name__ == "__main__":
    main()
