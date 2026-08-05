#!/usr/bin/env python3
"""Create a deterministic tar.gz or ZIP archive from a staging directory."""

from __future__ import annotations

import argparse
import gzip
import os
import stat
import tarfile
import tempfile
import zipfile
from pathlib import Path


def entries(source: Path):
    yield source
    yield from sorted(source.rglob("*"), key=lambda path: path.relative_to(source).as_posix())


def archive_tar(source: Path, output: Path, prefix: str) -> None:
    with output.open("wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w") as archive:
                for path in entries(source):
                    relative = path.relative_to(source).as_posix()
                    name = prefix if relative in ("", ".") else f"{prefix}/{relative}"
                    info = archive.gettarinfo(str(path), arcname=name)
                    info.mtime = 0
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    if path.is_file():
                        with path.open("rb") as payload:
                            archive.addfile(info, payload)
                    else:
                        archive.addfile(info)


def archive_zip(source: Path, output: Path, prefix: str) -> None:
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in entries(source):
            relative = path.relative_to(source).as_posix()
            name = prefix if relative in ("", ".") else f"{prefix}/{relative}"
            info = zipfile.ZipInfo(name + ("/" if path.is_dir() and not name.endswith("/") else ""))
            info.date_time = (1980, 1, 1, 0, 0, 0)
            info.create_system = 3
            mode = path.stat().st_mode
            info.external_attr = (stat.S_IMODE(mode) << 16) | (0x10 if path.is_dir() else 0)
            if path.is_file():
                archive.writestr(info, path.read_bytes())
            else:
                archive.writestr(info, b"")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--prefix", required=True)
    args = parser.parse_args()
    source = args.source.resolve()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary_file = tempfile.NamedTemporaryFile(
        prefix=f".{args.output.name}.",
        suffix=".tmp",
        dir=args.output.parent,
        delete=False,
    )
    temporary = Path(temporary_file.name)
    temporary_file.close()
    try:
        if args.output.name.endswith(".zip"):
            archive_zip(source, temporary, args.prefix)
        else:
            archive_tar(source, temporary, args.prefix)
        os.replace(temporary, args.output)
    except BaseException:
        temporary.unlink(missing_ok=True)
        raise
    print(f"Wrote reproducible archive {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
