#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import zipfile
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SOURCE = (
    Path.home()
    / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love"
)
DEFAULT_DEST = ROOT / "vendor/balatro/steam-local"
EXTRACT_PREFIXES = ("engine/", "functions/", "localization/")
EXTRACT_TOP_LEVEL_SUFFIX = ".lua"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Copy the local licensed Balatro package into the repo's ignored vendor folder and extract Lua sources"
    )
    parser.add_argument("--source", type=Path, default=DEFAULT_SOURCE)
    parser.add_argument("--dest", type=Path, default=DEFAULT_DEST)
    parser.add_argument("--force", action="store_true")
    return parser.parse_args()


def sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def should_extract(member: str) -> bool:
    if member.endswith("/") or member.startswith("__MACOSX/"):
        return False
    if "/" not in member:
        return member.endswith(EXTRACT_TOP_LEVEL_SUFFIX)
    return member.startswith(EXTRACT_PREFIXES)


def main() -> int:
    args = parse_args()
    if not args.source.exists():
        raise SystemExit(f"Balatro source package not found: {args.source}")

    dest = args.dest.resolve()
    original_dir = dest / "original"
    extracted_dir = dest / "extracted"
    manifest_path = dest / "manifest.json"
    copied_love = original_dir / "Balatro.love"

    if dest.exists() and args.force:
        shutil.rmtree(dest)

    original_dir.mkdir(parents=True, exist_ok=True)
    extracted_dir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(args.source, copied_love)

    extracted: list[str] = []
    with zipfile.ZipFile(copied_love) as archive:
        for member in sorted(archive.namelist()):
            if not should_extract(member):
                continue
            target = extracted_dir / member
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(member))
            extracted.append(member)

    manifest = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source_path": str(args.source),
        "copied_love_path": str(copied_love),
        "love_sha256": sha256_path(copied_love),
        "extracted_count": len(extracted),
        "extracted_entries": extracted,
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")

    print(f"copied {args.source} -> {copied_love}")
    print(f"sha256 {manifest['love_sha256']}")
    print(f"extracted {len(extracted)} source entries -> {extracted_dir}")
    print(f"wrote manifest -> {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
