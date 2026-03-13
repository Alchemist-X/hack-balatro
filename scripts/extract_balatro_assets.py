#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BUNDLE = ROOT / "fixtures/ruleset/balatro-1.0.1o-full.json"
DEFAULT_LOVE = Path.home() / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love"
DEFAULT_DEST = ROOT / "results/assets-preview"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Extract Balatro atlas files referenced by the bundle manifest")
    parser.add_argument("--bundle", type=Path, default=DEFAULT_BUNDLE)
    parser.add_argument("--love", type=Path, default=DEFAULT_LOVE)
    parser.add_argument("--dest", type=Path, default=DEFAULT_DEST)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    bundle = json.loads(args.bundle.read_text())
    manifest: dict[str, str] = bundle.get("sprite_manifest", {})
    args.dest.mkdir(parents=True, exist_ok=True)

    with zipfile.ZipFile(args.love) as archive:
        for archive_path in sorted(set(manifest.values())):
            target = args.dest / archive_path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive.read(archive_path))
            print(f"extracted {archive_path} -> {target}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
