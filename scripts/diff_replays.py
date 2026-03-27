#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path
from typing import Any


DEFAULT_IGNORES = {
    "_artifact_path",
    "test_metadata.session_id",
    "test_metadata.started_at",
    "test_metadata.finished_at",
    "transitions[].elapsed_ms",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Compare two replay artifacts field-by-field")
    parser.add_argument("--left", type=Path, required=True)
    parser.add_argument("--right", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--max-mismatches", type=int, default=200)
    return parser.parse_args()


def normalize_path(path: str) -> str:
    return path.replace("[", ".[").replace(".[", "[").strip(".")


def ignored(path: str) -> bool:
    normalized = normalize_path(path)
    if normalized in DEFAULT_IGNORES:
        return True
    return normalized.endswith(".elapsed_ms")


def compare(left: Any, right: Any, path: str, mismatches: list[dict[str, Any]], max_mismatches: int) -> None:
    if len(mismatches) >= max_mismatches or ignored(path):
        return
    if type(left) is not type(right):
        mismatches.append(
            {
                "path": path or "$",
                "category": "type",
                "left": left,
                "right": right,
            }
        )
        return
    if isinstance(left, dict):
        left_keys = set(left)
        right_keys = set(right)
        for key in sorted(left_keys | right_keys):
            child_path = f"{path}.{key}" if path else key
            if key not in left:
                mismatches.append({"path": child_path, "category": "missing_left", "left": None, "right": right[key]})
                continue
            if key not in right:
                mismatches.append({"path": child_path, "category": "missing_right", "left": left[key], "right": None})
                continue
            compare(left[key], right[key], child_path, mismatches, max_mismatches)
        return
    if isinstance(left, list):
        if len(left) != len(right):
            mismatches.append(
                {
                    "path": path or "$",
                    "category": "length",
                    "left": len(left),
                    "right": len(right),
                }
            )
            if len(mismatches) >= max_mismatches:
                return
        for index, (left_item, right_item) in enumerate(zip(left, right)):
            compare(left_item, right_item, f"{path}[{index}]", mismatches, max_mismatches)
        return
    if left != right:
        mismatches.append(
            {
                "path": path or "$",
                "category": "value",
                "left": left,
                "right": right,
            }
        )


def build_report(left: dict[str, Any], right: dict[str, Any], max_mismatches: int) -> dict[str, Any]:
    mismatches: list[dict[str, Any]] = []
    compare(left, right, "", mismatches, max_mismatches)
    categories = Counter(mismatch["category"] for mismatch in mismatches)
    return {
        "ok": not mismatches,
        "summary": {
            "left_path": str(left.get("_artifact_path", "")),
            "right_path": str(right.get("_artifact_path", "")),
            "mismatch_count": len(mismatches),
            "categories": dict(categories),
            "truncated": len(mismatches) >= max_mismatches,
        },
        "mismatches": mismatches,
    }


def main() -> int:
    args = parse_args()
    left = json.loads(args.left.read_text(encoding="utf-8"))
    right = json.loads(args.right.read_text(encoding="utf-8"))
    left["_artifact_path"] = str(args.left)
    right["_artifact_path"] = str(args.right)
    report = build_report(left, right, args.max_mismatches)
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote {args.output}")
    else:
        print(rendered, end="")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
