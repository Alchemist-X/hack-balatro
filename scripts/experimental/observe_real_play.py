#!/usr/bin/env python3
"""Passive observer: poll BalatroBot gamestate and log inferred user events.

Polls at 1 Hz, compares consecutive snapshots, and emits a JSONL event
for every meaningful transition (card played, discard, shop purchase,
state change, score increment, etc). Intended to run while the user
plays normally in the Balatro window.

Output:
  results/real-client-trajectories/observer-<ts>/events.jsonl
  results/real-client-trajectories/observer-<ts>/snapshots/<step>.json  (raw gamestate)
  results/real-client-trajectories/observer-<ts>/meta.json
"""
from __future__ import annotations

import argparse
import json
import signal
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


def rpc(method: str, params: dict[str, Any] | None = None, timeout: float = 5.0) -> Any:
    body: dict[str, Any] = {"jsonrpc": "2.0", "method": method, "id": 1}
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


def card_label(c: dict[str, Any]) -> str:
    v = c.get("value", {}) or {}
    rank = v.get("rank", "?")
    suit = v.get("suit", "?")
    mod = "/".join(m.get("key", m) if isinstance(m, dict) else str(m) for m in (c.get("modifier") or []))
    return f"{rank}{suit}" + (f"[{mod}]" if mod else "")


def summarize(state: dict[str, Any]) -> dict[str, Any]:
    hand = state.get("hand") or {}
    hand_cards = hand.get("cards") or []
    round_obj = state.get("round") or {}
    blinds = state.get("blinds") or {}
    def blind_status(key: str) -> str | None:
        b = blinds.get(key) if isinstance(blinds, dict) else None
        return b.get("status") if isinstance(b, dict) else None
    return {
        "state": state.get("state"),
        "ante": state.get("ante_num"),
        "round": state.get("round_num"),
        "hands_left": round_obj.get("hands_left") if isinstance(round_obj, dict) else None,
        "discards_left": round_obj.get("discards_left") if isinstance(round_obj, dict) else None,
        "round_chips": round_obj.get("chips") if isinstance(round_obj, dict) else None,
        "money": state.get("money"),
        "hand_count": len(hand_cards),
        "hand_cards": [card_label(c) for c in hand_cards],
        "hand_ids": [c.get("id") for c in hand_cards],
        "blind_small": blind_status("small"),
        "blind_big": blind_status("big"),
        "blind_boss": blind_status("boss"),
        "won": state.get("won"),
        "jokers": len((state.get("jokers") or {}).get("cards") or []) if isinstance(state.get("jokers"), dict) else None,
        "consumables": (state.get("consumables") or {}).get("count") if isinstance(state.get("consumables"), dict) else None,
    }


# states where "play a hand" or "discard" are meaningful actions
PLAY_PHASE_STATES = {"SELECTING_HAND", "HAND_PLAYED", "PLAY_TAROT", "DRAW_TO_HAND"}


def diff_events(prev: dict[str, Any], cur: dict[str, Any]) -> list[dict[str, Any]]:
    evs: list[dict[str, Any]] = []
    if prev["state"] != cur["state"]:
        evs.append({"event": "state_change", "from": prev["state"], "to": cur["state"]})
    # gate hand/discard inference to play-phase transitions so that round-boundary
    # counter resets (e.g. BLIND_SELECT -> DRAW_TO_HAND) do not look like actions
    in_play_phase = (prev["state"] in PLAY_PHASE_STATES) and (cur["state"] in PLAY_PHASE_STATES)
    # HAND PLAYED: hands_left dropped (while inside a round)
    if (
        in_play_phase
        and isinstance(prev["hands_left"], int)
        and isinstance(cur["hands_left"], int)
        and cur["hands_left"] < prev["hands_left"]
    ):
        dropped = [c for c in prev["hand_cards"] if c not in cur["hand_cards"]]
        gained = [c for c in cur["hand_cards"] if c not in prev["hand_cards"]]
        # at the exact moment hands_left decrements, the played cards are often
        # still rendered in-hand (pre-animation). adapter should reconcile by
        # inspecting the next tick with state ∈ {SELECTING_HAND, ROUND_EVAL}.
        evs.append({
            "event": "hand_played",
            "cards_played": dropped,
            "cards_drawn": gained,
            "hands_left_after": cur["hands_left"],
            "round_chips_after": cur["round_chips"],
            "needs_reconcile": not dropped,
            "pre_play_hand": prev["hand_cards"],
            "pre_play_hand_ids": prev["hand_ids"],
        })
    # DISCARD: discards_left dropped (while inside a round, not during round-boot resets)
    if (
        in_play_phase
        and isinstance(prev["discards_left"], int)
        and isinstance(cur["discards_left"], int)
        and cur["discards_left"] < prev["discards_left"]
        and prev["discards_left"] - cur["discards_left"] == 1  # exactly one discard at a time
    ):
        dropped = [c for c in prev["hand_cards"] if c not in cur["hand_cards"]]
        gained = [c for c in cur["hand_cards"] if c not in prev["hand_cards"]]
        evs.append({
            "event": "discard",
            "cards_discarded": dropped,
            "cards_drawn": gained,
            "discards_left_after": cur["discards_left"],
        })
    # ante / round
    if prev["ante"] != cur["ante"] and cur["ante"] is not None:
        evs.append({"event": "ante_change", "from": prev["ante"], "to": cur["ante"]})
    if prev["round"] != cur["round"] and cur["round"] is not None:
        evs.append({"event": "round_change", "from": prev["round"], "to": cur["round"]})
    # money delta (shop spend, cashout reward)
    if (
        isinstance(prev["money"], (int, float))
        and isinstance(cur["money"], (int, float))
        and prev["money"] != cur["money"]
    ):
        evs.append({
            "event": "money_change",
            "from": prev["money"],
            "to": cur["money"],
            "delta": cur["money"] - prev["money"],
        })
    # joker / consumable inventory change (buy/sell/use)
    if prev["jokers"] != cur["jokers"]:
        evs.append({"event": "jokers_count_change", "from": prev["jokers"], "to": cur["jokers"]})
    if prev["consumables"] != cur["consumables"]:
        evs.append({"event": "consumables_count_change", "from": prev["consumables"], "to": cur["consumables"]})
    # mid-round chip accumulation (scoring animation)
    if (
        isinstance(prev["round_chips"], (int, float))
        and isinstance(cur["round_chips"], (int, float))
        and cur["round_chips"] > prev["round_chips"]
        and prev["state"] == cur["state"]
    ):
        evs.append({"event": "round_chips_up", "from": prev["round_chips"], "to": cur["round_chips"]})
    # blind progression
    for b in ("small", "big", "boss"):
        k = f"blind_{b}"
        if prev[k] != cur[k]:
            evs.append({"event": "blind_status_change", "blind": b, "from": prev[k], "to": cur[k]})
    return evs


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--session", default=f"observer-{datetime.now().strftime('%Y%m%dT%H%M%S')}")
    p.add_argument("--interval", type=float, default=0.2, help="seconds between gamestate polls (default 5 Hz)")
    p.add_argument("--max-minutes", type=float, default=60.0)
    p.add_argument("--snapshot-every", type=int, default=50, help="Write full raw snapshot every N ticks (0=never)")
    args = p.parse_args()

    session_dir = Path("results/real-client-trajectories") / args.session
    session_dir.mkdir(parents=True, exist_ok=True)
    snap_dir = session_dir / "snapshots"
    snap_dir.mkdir(exist_ok=True)
    events_path = session_dir / "events.jsonl"
    meta_path = session_dir / "meta.json"

    meta = {"session": args.session, "started_at": now_iso(), "poll_interval_s": args.interval}
    meta_path.write_text(json.dumps(meta, indent=2))

    stop = {"flag": False}
    def on_sig(signum, frame):
        stop["flag"] = True
    signal.signal(signal.SIGINT, on_sig)
    signal.signal(signal.SIGTERM, on_sig)

    def emit(ev_kind: str, payload: dict[str, Any]) -> None:
        rec = {"ts": now_iso(), "kind": ev_kind, **payload}
        with events_path.open("a") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        f_payload = {k: v for k, v in payload.items() if k != "raw"}
        print(f"[{rec['ts']}] {ev_kind}: {json.dumps(f_payload, ensure_ascii=False)}", flush=True)

    print(f"[observer] session={args.session} interval={args.interval}s dir={session_dir}", flush=True)

    # initial health probe
    try:
        h = rpc("health")
        emit("health", h)
    except Exception as e:  # noqa: BLE001
        emit("error", {"where": "initial_health", "error": str(e)})
        return 2

    prev_summary: dict[str, Any] | None = None
    tick = 0
    deadline = time.monotonic() + args.max_minutes * 60.0

    while not stop["flag"] and time.monotonic() < deadline:
        tick += 1
        try:
            state = rpc("gamestate")
        except Exception as e:  # noqa: BLE001
            emit("error", {"where": "gamestate", "error": str(e), "tick": tick})
            time.sleep(args.interval)
            continue

        summary = summarize(state)

        if prev_summary is None:
            emit("initial", {"summary": summary})
        else:
            evs = diff_events(prev_summary, summary)
            for ev in evs:
                emit(ev["event"], {**ev, "summary_after": summary})

        if args.snapshot_every > 0 and tick % args.snapshot_every == 0:
            (snap_dir / f"tick-{tick:06d}.json").write_text(json.dumps(state, ensure_ascii=False))

        prev_summary = summary
        time.sleep(args.interval)

    meta = json.loads(meta_path.read_text())
    meta["ended_at"] = now_iso()
    meta["total_ticks"] = tick
    meta_path.write_text(json.dumps(meta, indent=2))

    print(f"[observer] stopped. ticks={tick} events_file={events_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
