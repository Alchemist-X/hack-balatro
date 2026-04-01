#!/usr/bin/env python3
"""Play one Balatro game interactively via stdout/stdin.

Designed to be called by an LLM subagent. Outputs game state as JSON,
reads action decisions from the agent, records full trajectory.

Usage (by subagent):
    This script is NOT run directly. The subagent imports and calls play_game().
"""
from __future__ import annotations
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import balatro_native
from env.state_serializer import serialize_state

RANKS = ["2","3","4","5","6","7","8","9","10","J","Q","K","A"]
SUITS = ["Spades","Hearts","Diamonds","Clubs"]
RANK_SHORT = {"Two":"2","Three":"3","Four":"4","Five":"5","Six":"6","Seven":"7",
              "Eight":"8","Nine":"9","Ten":"10","Jack":"J","Queen":"Q","King":"K","Ace":"A"}

def card_label(c):
    r = RANK_SHORT.get(c.get("rank",""), c.get("rank","?"))
    s = c.get("suit","?")[0] if c.get("suit") else "?"
    return f"{r}{s}"

def get_state(eng):
    """Get full game state as dict."""
    snap = eng.snapshot()
    d = json.loads(snap.to_json())
    acts = [a for a in eng.legal_actions() if a.enabled]
    legal = [a.name for a in acts]
    state_text = serialize_state(d, legal)
    return {
        "snapshot": d,
        "legal_actions": legal,
        "state_text": state_text,
        "is_over": bool(eng.is_over),
        "is_win": bool(eng.is_win),
    }

def execute_action(eng, action_name):
    """Execute an action by name. Returns True if successful."""
    for a in eng.legal_actions():
        if a.enabled and a.name == action_name:
            eng.step(a.index)
            return True
    return False

def play_game(seed, max_steps=500):
    """Play a full game. Returns trajectory list."""
    eng = balatro_native.Engine(seed=seed, stake=1)
    return eng, get_state(eng)
