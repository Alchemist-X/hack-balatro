#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Probe local Balatro real-client capture prerequisites")
    parser.add_argument("--output", type=Path, default=Path("results/real-client-bootstrap.json"))
    parser.add_argument(
        "--session-root",
        type=Path,
        default=Path("results/real-client-trajectories"),
        help="Target root for future real-client trajectory sessions",
    )
    parser.add_argument(
        "--skip-sha256",
        action="store_true",
        help="Skip hashing Balatro.love when a faster probe is preferred",
    )
    return parser.parse_args()


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def sha256_file(path: Path) -> str | None:
    if not path.exists() or not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def detect_paths() -> dict[str, Path]:
    home = Path.home()
    if sys.platform == "darwin":
        return {
            "game_dir": home / "Library/Application Support/Steam/steamapps/common/Balatro",
            "app_bundle": home / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app",
            "love_binary": home / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/MacOS/love",
            "balatro_love": home
            / "Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love",
            "save_dir": home / "Library/Application Support/Balatro",
            "mods_dir": home / "Library/Application Support/Balatro/Mods",
            "lovely_dylib": home / "Library/Application Support/Steam/steamapps/common/Balatro/liblovely.dylib",
            "lovely_launcher": home
            / "Library/Application Support/Steam/steamapps/common/Balatro/run_lovely_macos.sh",
        }
    if sys.platform.startswith("linux"):
        return {
            "game_dir": home / ".local/share/Steam/steamapps/common/Balatro",
            "love_binary": Path(shutil.which("love") or ""),
            "balatro_love": home / ".local/share/Steam/steamapps/common/Balatro/Balatro.love",
            "save_dir": home / ".local/share/love/balatro",
            "mods_dir": home / ".config/love/Mods",
            "lovely_so": Path("/usr/local/lib/liblovely.so"),
        }
    appdata = Path.home()
    return {
        "game_dir": Path(r"C:\Program Files (x86)\Steam\steamapps\common\Balatro"),
        "love_binary": Path(r"C:\Program Files (x86)\Steam\steamapps\common\Balatro\Balatro.exe"),
        "balatro_love": Path(r"C:\Program Files (x86)\Steam\steamapps\common\Balatro\Balatro.exe"),
        "save_dir": appdata / "AppData/Roaming/Balatro",
        "mods_dir": appdata / "AppData/Roaming/Balatro/Mods",
        "lovely_dll": Path(r"C:\Program Files (x86)\Steam\steamapps\common\Balatro\version.dll"),
    }


def list_files(root: Path) -> list[dict[str, Any]]:
    if not root.exists():
        return []
    entries: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        stat = path.stat()
        entries.append(
            {
                "relative_path": str(path.relative_to(root)),
                "size_bytes": stat.st_size,
                "mtime": datetime.fromtimestamp(stat.st_mtime, timezone.utc).isoformat(),
            }
        )
    return entries


def build_launch_instructions(paths: dict[str, Path], session_root: Path) -> dict[str, Any]:
    instructions: dict[str, Any] = {
        "health_check_curl": (
            "curl -X POST http://127.0.0.1:12346 "
            "-H 'Content-Type: application/json' "
            "-d '{\"jsonrpc\":\"2.0\",\"method\":\"health\",\"id\":1}'"
        ),
        "manual_recorder": (
            "python3 scripts/record_manual_real_trajectory.py "
            f"--session-dir {session_root / 'manual-demo'} --deck RED --stake WHITE --seed 123456"
        ),
    }
    if sys.platform == "darwin":
        instructions["launch_balatrobot"] = "uvx balatrobot serve --fast"
        instructions["launch_lovely_direct"] = (
            f"cd '{paths['game_dir']}' && sh run_lovely_macos.sh"
        )
        instructions["notes"] = [
            "macOS 上不能通过 Steam 直接带着 Lovely 启动 Balatro；应通过 BalatroBot CLI 或 run_lovely_macos.sh 启动。",
            "BalatroBot 默认监听 http://127.0.0.1:12346 。",
        ]
    elif sys.platform.startswith("linux"):
        instructions["launch_balatrobot"] = "uvx balatrobot serve --platform native --fast"
    else:
        instructions["launch_balatrobot"] = "uvx balatrobot serve --fast"
    return instructions


def main() -> int:
    args = parse_args()
    paths = detect_paths()
    session_root = args.session_root.resolve()
    session_root.mkdir(parents=True, exist_ok=True)

    path_report: dict[str, Any] = {}
    missing: list[str] = []
    for name, path in paths.items():
        exists = path.exists()
        path_report[name] = {
            "path": str(path),
            "exists": exists,
        }
        if exists and path.is_file():
            path_report[name]["size_bytes"] = path.stat().st_size
        if not exists:
            missing.append(name)

    love_path = paths.get("balatro_love")
    love_sha256 = None if args.skip_sha256 else sha256_file(love_path) if love_path else None

    command_report = {
        "uv": shutil.which("uv"),
        "uvx": shutil.which("uvx"),
        "python3": shutil.which("python3"),
        "python3.13": shutil.which("python3.13"),
    }
    if command_report["uv"] is None and command_report["uvx"] is None:
        missing.append("uv")

    if not paths.get("mods_dir", Path()).exists():
        missing.append("mods_dir")
    if sys.platform == "darwin":
        if not paths["lovely_dylib"].exists():
            missing.append("lovely_dylib")
        if not paths["lovely_launcher"].exists():
            missing.append("lovely_launcher")

    report = {
        "captured_at": now_iso(),
        "platform": sys.platform,
        "cwd": str(Path.cwd()),
        "paths": path_report,
        "commands": command_report,
        "save_dir_files": list_files(paths["save_dir"]) if "save_dir" in paths else [],
        "balatro_love_sha256": love_sha256,
        "results_root": str(session_root),
        "capture_ready": len(set(missing)) == 0,
        "missing_prerequisites": sorted(set(missing)),
        "recommended_commands": build_launch_instructions(paths, session_root),
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(f"wrote {args.output}")
    print(f"capture_ready={report['capture_ready']}")
    if report["missing_prerequisites"]:
        print("missing=" + ", ".join(report["missing_prerequisites"]))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
