#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_HOST = "127.0.0.1"
DEFAULT_PORT = 12346


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Record a human-played real Balatro trajectory via BalatroBot")
    parser.add_argument("--host", default=DEFAULT_HOST)
    parser.add_argument("--port", type=int, default=DEFAULT_PORT)
    parser.add_argument(
        "--session-dir",
        type=Path,
        default=Path("results/real-client-trajectories/manual-session"),
    )
    parser.add_argument("--deck", default=None, help="Optional deck for a BalatroBot start call, e.g. RED")
    parser.add_argument("--stake", default=None, help="Optional stake for a BalatroBot start call, e.g. WHITE")
    parser.add_argument("--seed", default=None, help="Optional seed passed to BalatroBot start")
    parser.add_argument("--skip-save", action="store_true", help="Do not call the save RPC during capture")
    parser.add_argument("--skip-screenshot", action="store_true", help="Do not call the screenshot RPC during capture")
    parser.add_argument("--label-prefix", default="manual")
    return parser.parse_args()


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def rpc_payload(method: str, params: dict[str, Any] | None = None, request_id: int = 1) -> dict[str, Any]:
    payload: dict[str, Any] = {"jsonrpc": "2.0", "method": method, "id": request_id}
    if params:
        payload["params"] = params
    return payload


def rpc_call(host: str, port: int, method: str, params: dict[str, Any] | None = None, request_id: int = 1) -> dict[str, Any]:
    payload = rpc_payload(method=method, params=params, request_id=request_id)
    data = json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        url=f"http://{host}:{port}",
        data=data,
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            raw = response.read().decode("utf-8")
    except urllib.error.URLError as exc:
        raise RuntimeError(f"rpc {method} failed: {exc}") from exc

    envelope = json.loads(raw)
    if "error" in envelope:
        error = envelope["error"]
        name = error.get("data", {}).get("name", "RPC_ERROR")
        raise RuntimeError(f"rpc {method} returned {name}: {error.get('message')}")
    return envelope


def write_json(path: Path, payload: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def maybe_decode_screenshot(result: Any, png_path: Path) -> str | None:
    if isinstance(result, str):
        candidates = [result]
    elif isinstance(result, dict):
        candidates = [
            value
            for key, value in result.items()
            if key in {"image", "image_base64", "png", "png_base64", "data"} and isinstance(value, str)
        ]
    else:
        candidates = []

    for candidate in candidates:
        try:
            decoded = base64.b64decode(candidate, validate=True)
        except Exception:
            continue
        if decoded.startswith(b"\x89PNG\r\n\x1a\n"):
            png_path.parent.mkdir(parents=True, exist_ok=True)
            png_path.write_bytes(decoded)
            return str(png_path)
    return None


def find_key(data: Any, target: str, depth: int = 0) -> Any | None:
    if depth > 3:
        return None
    if isinstance(data, dict):
        if target in data:
            return data[target]
        for value in data.values():
            found = find_key(value, target, depth + 1)
            if found is not None:
                return found
    elif isinstance(data, list):
        for value in data[:16]:
            found = find_key(value, target, depth + 1)
            if found is not None:
                return found
    return None


def summarize_gamestate(gamestate: Any) -> str:
    if not isinstance(gamestate, dict):
        return f"type={type(gamestate).__name__}"
    fields: list[str] = []
    for key in [
        "state",
        "screen",
        "blind",
        "blind_name",
        "ante",
        "money",
        "dollars",
        "round",
        "hands",
        "discards",
        "seed",
    ]:
        value = find_key(gamestate, key)
        if value is not None:
            fields.append(f"{key}={value}")
    if not fields:
        keys = ", ".join(list(gamestate.keys())[:10])
        return f"top_level_keys={keys}"
    return ", ".join(fields)


def capture_bundle(
    *,
    host: str,
    port: int,
    session_dir: Path,
    capture_name: str,
    action_label: str,
    note: str,
    request_id: int,
    do_save: bool,
    do_screenshot: bool,
) -> dict[str, Any]:
    rpc_dir = session_dir / "rpc"
    step_dir = session_dir / "steps"
    screenshot_dir = session_dir / "screenshots"

    gamestate_envelope = rpc_call(host, port, "gamestate", request_id=request_id)
    gamestate_path = step_dir / f"{capture_name}.gamestate.json"
    gamestate_rpc_path = rpc_dir / f"{capture_name}.gamestate.rpc.json"
    write_json(gamestate_path, gamestate_envelope["result"])
    write_json(gamestate_rpc_path, gamestate_envelope)

    save_rpc_path = None
    if do_save:
        save_envelope = rpc_call(host, port, "save", request_id=request_id + 1000)
        save_rpc_path = rpc_dir / f"{capture_name}.save.rpc.json"
        write_json(save_rpc_path, save_envelope)

    screenshot_rpc_path = None
    screenshot_png_path = None
    if do_screenshot:
        screenshot_envelope = rpc_call(host, port, "screenshot", request_id=request_id + 2000)
        screenshot_rpc_path = rpc_dir / f"{capture_name}.screenshot.rpc.json"
        write_json(screenshot_rpc_path, screenshot_envelope)
        png_path = screenshot_dir / f"{capture_name}.png"
        decoded = maybe_decode_screenshot(screenshot_envelope.get("result"), png_path)
        if decoded is not None:
            screenshot_png_path = Path(decoded)

    return {
        "captured_at": now_iso(),
        "capture_name": capture_name,
        "action_label": action_label,
        "note": note,
        "summary": summarize_gamestate(gamestate_envelope["result"]),
        "files": {
            "gamestate": str(gamestate_path.relative_to(session_dir)),
            "gamestate_rpc": str(gamestate_rpc_path.relative_to(session_dir)),
            "save_rpc": str(save_rpc_path.relative_to(session_dir)) if save_rpc_path else None,
            "screenshot_rpc": str(screenshot_rpc_path.relative_to(session_dir)) if screenshot_rpc_path else None,
            "screenshot_png": str(screenshot_png_path.relative_to(session_dir)) if screenshot_png_path else None,
        },
        "gamestate": gamestate_envelope["result"],
    }


def update_manifest(session_dir: Path, manifest: dict[str, Any]) -> None:
    write_json(session_dir / "session_manifest.json", manifest)
    summary = {
        "captured_at": now_iso(),
        "status": manifest["status"],
        "step_count": len(manifest["steps"]),
        "seed": manifest.get("seed"),
        "latest_step": manifest["steps"][-1] if manifest["steps"] else None,
    }
    write_json(session_dir / "session_summary.json", summary)


def main() -> int:
    args = parse_args()
    session_dir = args.session_dir.resolve()
    session_dir.mkdir(parents=True, exist_ok=True)

    manifest: dict[str, Any] = {
        "version": 1,
        "created_at": now_iso(),
        "status": "bootstrapping",
        "transport": {"host": args.host, "port": args.port},
        "session_dir": str(session_dir),
        "seed": args.seed,
        "start_request": None,
        "steps": [],
    }
    update_manifest(session_dir, manifest)

    try:
        health_envelope = rpc_call(args.host, args.port, "health", request_id=1)
    except RuntimeError as exc:
        manifest["status"] = "not_connected"
        manifest["connection_error"] = str(exc)
        update_manifest(session_dir, manifest)
        print(f"failed to connect: {exc}", file=sys.stderr)
        print(f"wrote bootstrap manifest to {session_dir / 'session_manifest.json'}")
        return 2

    write_json(session_dir / "rpc" / "000.health.rpc.json", health_envelope)
    manifest["health"] = health_envelope["result"]
    manifest["status"] = "connected"
    update_manifest(session_dir, manifest)

    request_id = 10
    if args.deck or args.stake or args.seed:
        start_params: dict[str, Any] = {}
        if args.deck:
            start_params["deck"] = args.deck
        if args.stake:
            start_params["stake"] = args.stake
        if args.seed:
            start_params["seed"] = args.seed
        start_envelope = rpc_call(args.host, args.port, "start", start_params, request_id=request_id)
        write_json(session_dir / "rpc" / "001.start.rpc.json", start_envelope)
        manifest["start_request"] = start_params
        manifest["status"] = "recording"
        start_capture = {
            "captured_at": now_iso(),
            "capture_name": "001-start-run",
            "action_label": "start_run",
            "note": "Started a new run through BalatroBot before manual play.",
            "summary": summarize_gamestate(start_envelope["result"]),
            "files": {
                "gamestate": None,
                "gamestate_rpc": str(Path("rpc") / "001.start.rpc.json"),
                "save_rpc": None,
                "screenshot_rpc": None,
                "screenshot_png": None,
            },
            "gamestate": start_envelope["result"],
        }
        manifest["steps"].append(start_capture)
        update_manifest(session_dir, manifest)
        request_id += 1

    initial = capture_bundle(
        host=args.host,
        port=args.port,
        session_dir=session_dir,
        capture_name="002-initial",
        action_label="capture_initial",
        note="Initial snapshot before the human performs the first manual action.",
        request_id=request_id,
        do_save=not args.skip_save,
        do_screenshot=not args.skip_screenshot,
    )
    manifest["steps"].append(initial)
    manifest["status"] = "recording"
    update_manifest(session_dir, manifest)
    print(f"[captured] {initial['capture_name']}: {initial['summary']}")
    request_id += 1

    step_index = 3
    while True:
        raw_action = input("action label (q to finish): ").strip()
        if raw_action.lower() in {"q", "quit", "exit", "done"}:
            break
        if not raw_action:
            continue
        note = input("note (optional): ").strip()
        input("在真实客户端执行完这个动作后按 Enter 采集快照...")
        capture_name = f"{step_index:03d}-{args.label_prefix}"
        record = capture_bundle(
            host=args.host,
            port=args.port,
            session_dir=session_dir,
            capture_name=capture_name,
            action_label=raw_action,
            note=note,
            request_id=request_id,
            do_save=not args.skip_save,
            do_screenshot=not args.skip_screenshot,
        )
        manifest["steps"].append(record)
        update_manifest(session_dir, manifest)
        print(f"[captured] {capture_name}: {record['summary']}")
        step_index += 1
        request_id += 1

    final = capture_bundle(
        host=args.host,
        port=args.port,
        session_dir=session_dir,
        capture_name=f"{step_index:03d}-final",
        action_label="capture_final",
        note="Final snapshot after the human ended the manual session.",
        request_id=request_id,
        do_save=not args.skip_save,
        do_screenshot=not args.skip_screenshot,
    )
    manifest["steps"].append(final)
    manifest["status"] = "completed"
    update_manifest(session_dir, manifest)
    print(f"[captured] {final['capture_name']}: {final['summary']}")
    print(f"wrote session to {session_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
