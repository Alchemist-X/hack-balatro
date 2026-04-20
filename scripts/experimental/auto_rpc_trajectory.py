#!/usr/bin/env python3
"""Drive real Balatro (via BalatroBot RPC) with a trivial policy, record trajectory, save, verify.

One-shot sanity check that the Lovely + Steamodded + BalatroBot + RPC chain
works end-to-end on this machine, and that trajectories land on disk in a
shape we can read back.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


HOST = "127.0.0.1"
PORT = 12346


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def rpc(method: str, params: dict[str, Any] | None = None, timeout: float = 10.0) -> Any:
    body = {"jsonrpc": "2.0", "method": method, "id": 1}
    if params is not None:
        body["params"] = params
    req = urllib.request.Request(
        url=f"http://{HOST}:{PORT}",
        data=json.dumps(body).encode(),
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        envelope = json.loads(resp.read().decode())
    if "error" in envelope:
        raise RuntimeError(f"rpc {method} error: {envelope['error']}")
    return envelope["result"]


def find_key(obj: Any, target: str, depth: int = 0) -> Any:
    if depth > 8:
        return None
    if isinstance(obj, dict):
        if target in obj:
            return obj[target]
        for v in obj.values():
            got = find_key(v, target, depth + 1)
            if got is not None:
                return got
    elif isinstance(obj, list):
        for v in obj:
            got = find_key(v, target, depth + 1)
            if got is not None:
                return got
    return None


def summarize(state: Any) -> dict[str, Any]:
    return {
        "state": find_key(state, "state"),
        "ui_state": find_key(state, "ui_state"),
        "ante": find_key(state, "ante"),
        "round": find_key(state, "round"),
        "chips": find_key(state, "chips"),
        "mult": find_key(state, "mult"),
        "hands": find_key(state, "hands"),
        "discards": find_key(state, "discards"),
        "dollars": find_key(state, "dollars"),
    }


def wait_for_state(
    match: callable,
    *,
    max_wait_s: float = 30.0,
    poll_s: float = 0.5,
    label: str = "state",
) -> Any:
    deadline = time.monotonic() + max_wait_s
    last = None
    while time.monotonic() < deadline:
        try:
            state = rpc("gamestate")
        except Exception as e:  # noqa: BLE001
            print(f"  [wait] gamestate transient error: {e}", file=sys.stderr)
            time.sleep(poll_s)
            continue
        if match(state):
            return state
        last = summarize(state)
        time.sleep(poll_s)
    raise TimeoutError(f"timeout waiting for {label}; last summary={last}")


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--deck", default="RED")
    p.add_argument("--stake", default="WHITE")
    p.add_argument("--seed", default="AUTO20260420")
    p.add_argument("--max-steps", type=int, default=15)
    p.add_argument(
        "--output",
        type=Path,
        default=Path("results/real-client-trajectories/auto-rpc-first")
        / f"trajectory-{datetime.now().strftime('%Y%m%dT%H%M%SZ')}.json",
    )
    args = p.parse_args()

    print(f"[0] health")
    print(f"    {rpc('health')}")

    print(f"[1] navigate to menu (in case a run is already in progress)")
    try:
        rpc("menu")
    except Exception as e:  # noqa: BLE001
        print(f"    menu rpc raised (safe to ignore if already at menu): {e}")
    time.sleep(1.5)

    print(f"[2] start new run: deck={args.deck} stake={args.stake} seed={args.seed}")
    start_result = rpc(
        "start",
        {"deck": args.deck, "stake": args.stake, "seed": args.seed},
    )
    print(f"    start ack -> keys={list(start_result.keys()) if isinstance(start_result, dict) else type(start_result).__name__}")

    print(f"[3] wait for selecting-blind UI")
    _ = wait_for_state(
        lambda s: find_key(s, "state") in {"BLIND_SELECT", 7}
        or (find_key(s, "ui_state") or "").upper() == "BLIND_SELECT",
        label="BLIND_SELECT",
        max_wait_s=20.0,
    )

    print(f"[4] select small blind -> play 5 cards -> discard -> repeat, up to {args.max_steps} actions")
    traj: list[dict[str, Any]] = []

    # 4a. Confirm small blind
    try:
        rpc("select", {"blind": "small"})
    except Exception as e:
        print(f"    select small blind failed: {e}")
        try:
            rpc("select")
        except Exception as e2:
            print(f"    raw select failed too: {e2}")

    time.sleep(1.5)
    selecting = wait_for_state(
        lambda s: find_key(s, "state") in {"SELECTING_HAND", 1}
        or (find_key(s, "ui_state") or "").upper() == "SELECTING_HAND",
        label="SELECTING_HAND",
        max_wait_s=20.0,
    )
    print(f"    entered SELECTING_HAND, summary={summarize(selecting)}")

    for step in range(args.max_steps):
        try:
            state = rpc("gamestate")
        except Exception as e:  # noqa: BLE001
            print(f"  step {step}: gamestate failed: {e}; stopping")
            break

        summary = summarize(state)
        action: dict[str, Any]

        cur_state = summary["state"]
        cur_ui = (summary["ui_state"] or "").upper() if summary["ui_state"] else ""

        if cur_state in {"SELECTING_HAND", 1} or cur_ui == "SELECTING_HAND":
            hands_left = summary["hands"] or 0
            discards_left = summary["discards"] or 0
            if hands_left > 0:
                action = {"method": "play", "params": {"cards": [0, 1, 2, 3, 4]}}
            elif discards_left > 0:
                action = {"method": "discard", "params": {"cards": [0, 1, 2]}}
            else:
                action = {"method": "gamestate"}  # nothing to do, just observe
        elif cur_state in {"ROUND_EVAL", 6} or cur_ui == "ROUND_EVAL":
            action = {"method": "cash_out"}
        elif cur_state in {"SHOP", 5} or cur_ui == "SHOP":
            action = {"method": "next_round"}
        elif cur_ui in {"BLIND_SELECT"}:
            action = {"method": "select", "params": {"blind": "small"}}
        else:
            action = {"method": "gamestate"}

        try:
            result = rpc(action["method"], action.get("params"))
            ok, err = True, None
        except Exception as e:  # noqa: BLE001
            ok, err, result = False, str(e), None

        entry = {
            "step": step,
            "timestamp": now_iso(),
            "summary_before": summary,
            "action": action,
            "ok": ok,
            "error": err,
        }
        traj.append(entry)
        print(
            f"  step {step:02d}: state={cur_state} ui={cur_ui or '-'} "
            f"hands={summary['hands']} discards={summary['discards']} "
            f"chips={summary['chips']} -> {action['method']} ok={ok}"
            + (f" err={err}" if err else "")
        )

        if not ok and err and "GAME_OVER" in err:
            print("  run ended (GAME_OVER)")
            break

        time.sleep(1.2)

    print(f"[5] save trajectory to {args.output}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "meta": {
            "tool": "auto_rpc_trajectory",
            "captured_at": now_iso(),
            "deck": args.deck,
            "stake": args.stake,
            "seed": args.seed,
            "step_count": len(traj),
        },
        "trajectory": traj,
    }
    args.output.write_text(json.dumps(payload, ensure_ascii=False, indent=2))

    print(f"[6] verify by re-reading")
    reloaded = json.loads(args.output.read_text())
    assert reloaded["meta"]["step_count"] == len(traj), "step count mismatch"
    assert len(reloaded["trajectory"]) == len(traj), "traj len mismatch"
    print(f"    OK: {len(traj)} steps, file {args.output.stat().st_size} bytes")

    ok_steps = sum(1 for s in traj if s["ok"])
    print(f"[7] summary: {ok_steps}/{len(traj)} actions succeeded")
    return 0


if __name__ == "__main__":
    sys.exit(main())
