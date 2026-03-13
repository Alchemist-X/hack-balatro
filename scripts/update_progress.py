#!/usr/bin/env python3
from __future__ import annotations

import argparse
from datetime import datetime
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Append a rough-loop entry to progress.md")
    parser.add_argument("--completed", action="append", default=[], help="Completed item")
    parser.add_argument("--in-progress", dest="in_progress", action="append", default=[], help="In-progress item")
    parser.add_argument("--next", action="append", default=[], help="Next item")
    parser.add_argument("--need-help", dest="need_help", action="append", default=[], help="Human help request")
    parser.add_argument("--check", action="append", default=[], help="Checklist entry in the form '[x] text' or '[ ] text'")
    parser.add_argument("--path", type=Path, default=Path("progress.md"))
    return parser.parse_args()


def bullet_lines(items: list[str]) -> str:
    if not items:
        return "- None\n"
    return "".join(f"- {item}\n" for item in items)


def checklist_lines(items: list[str]) -> str:
    if not items:
        return "- [ ] No checklist entries supplied\n"
    return "".join(f"- {item}\n" if item.startswith("[") else f"- [ ] {item}\n" for item in items)


def main() -> int:
    args = parse_args()
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    block = (
        f"\n## {timestamp}\n\n"
        f"### Completed\n{bullet_lines(args.completed)}\n"
        f"### In Progress\n{bullet_lines(args.in_progress)}\n"
        f"### Next\n{bullet_lines(args.next)}\n"
        f"### Checklist\n{checklist_lines(args.check)}\n"
        f"### Need Human Help\n{bullet_lines(args.need_help)}"
    )
    args.path.write_text(args.path.read_text() + block if args.path.exists() else f"# Progress\n{block}")
    print(f"updated {args.path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
