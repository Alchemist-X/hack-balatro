#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render a standalone autoplay replay HTML from replay JSON")
    parser.add_argument("--replay", type=Path, required=True)
    parser.add_argument("--viewer", type=Path, default=Path("viewer/index.html"))
    parser.add_argument("--output", type=Path, default=Path("results/replay-latest.html"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    replay = json.loads(args.replay.read_text())
    html = args.viewer.read_text()
    injection = (
        "<script>\n"
        f"window.__REPLAY__ = {json.dumps(replay, ensure_ascii=False)};\n"
        "window.__AUTO_PLAY__ = true;\n"
        "</script>\n"
    )
    out = html.replace("</head>", f"{injection}</head>", 1)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(out)
    print(f"wrote {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
