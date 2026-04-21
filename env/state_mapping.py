"""Sim ⇄ Real-Client schema mapping.

The BalatroBot RPC (`gamestate` method) returns the authoritative Lua-side
shape. Our Rust simulator emits a flatter, Rust-idiomatic shape via
`PySnapshot.to_json()`. This module converts the latter to the former so the
two can be compared field-by-field and consumed by the same downstream code.

Source of truth for the real shape:
    results/real-client-trajectories/observer-20260420T223706/snapshots/tick-000010.json

Produced canonical fields (subset — expand as we go):
    state, ante_num, round_num, seed, deck, stake, won, money,
    round: { hands_left, hands_played, discards_left, discards_used, chips,
             reroll_cost },
    hand: { cards, count, limit, highlighted_limit },
    jokers: { cards, count, limit, highlighted_limit },
    consumables: { cards, count, limit, highlighted_limit },
    blinds: { small|big|boss: { status, score, name, effect,
                                tag_name, tag_effect, type } },
    used_vouchers: [...], vouchers: [...],
    packs: [...], shop: { jokers, consumables, vouchers, cards }.
"""
from __future__ import annotations

from typing import Any

# --- stake integer ↔ BalatroBot uppercase string --------------------------
STAKE_BY_INT: dict[int, str] = {
    1: "WHITE",
    2: "RED",
    3: "GREEN",
    4: "BLACK",
    5: "BLUE",
    6: "PURPLE",
    7: "ORANGE",
    8: "GOLD",
}
INT_BY_STAKE: dict[str, int] = {v: k for k, v in STAKE_BY_INT.items()}


# --- BlindState TitleCase ↔ BalatroBot UPPERCASE --------------------------
BLIND_STATE_TO_REAL: dict[str, str] = {
    "Upcoming": "UPCOMING",
    "Select": "SELECT",
    "Current": "CURRENT",
    "Defeated": "DEFEATED",
    "Skipped": "SKIPPED",
}
BLIND_STATE_FROM_REAL: dict[str, str] = {v: k for k, v in BLIND_STATE_TO_REAL.items()}


# --- sim `stage` ↔ real `state` (the BalatroBot `gamestate.state`) --------
# These are the high-level Lua phases used by the game's own state machine.
# The sim also emits `lua_state` which should be identity-aligned already.
SIM_STAGE_TO_LUA_STATE: dict[str, str] = {
    "Stage_PreBlind": "BLIND_SELECT",
    "Stage_Blind": "SELECTING_HAND",
    "Stage_Scoring": "HAND_PLAYED",
    "Stage_RoundEval": "ROUND_EVAL",
    "Stage_Shop": "SHOP",
    "Stage_GameOver": "GAME_OVER",
}


# --- suit + rank canonicalization -----------------------------------------
SUIT_CODE: dict[str, str] = {
    "Spades": "S", "Hearts": "H", "Clubs": "C", "Diamonds": "D",
    "Spade": "S", "Heart": "H", "Club": "C", "Diamond": "D",
    "S": "S", "H": "H", "C": "C", "D": "D",
}
RANK_CODE: dict[str, str] = {
    "Ace": "A", "King": "K", "Queen": "Q", "Jack": "J",
    "Ten": "T", "Nine": "9", "Eight": "8", "Seven": "7",
    "Six": "6", "Five": "5", "Four": "4", "Three": "3", "Two": "2",
    # digit/letter forms pass through
    **{c: c for c in "A23456789TJQK"},
    "10": "T",
}


def card_label(card: Any) -> str:
    """Produce a `RankSuit` label like real client (`TH`, `9C`, `AS`)."""
    if isinstance(card, dict):
        rank = card.get("rank") or card.get("value", {}).get("rank")
        suit = card.get("suit") or card.get("value", {}).get("suit")
    else:
        rank = getattr(card, "rank", None)
        suit = getattr(card, "suit", None)
    r = RANK_CODE.get(str(rank), "?")
    s = SUIT_CODE.get(str(suit), "?")
    return f"{r}{s}"


def blind_target(round_num: int, ante_num: int, kind: str) -> int:
    """Base-chip target per blind kind at a given ante.

    Vanilla Balatro scaling (stakes above White apply further multipliers):
        Small   = base[ante]
        Big     = base[ante] * 1.5
        Boss    = base[ante] * 2
    """
    base = {1: 300, 2: 600, 3: 1000, 4: 1800, 5: 3200, 6: 5600, 7: 10000, 8: 20000}
    b = base.get(ante_num, base[8])
    if kind == "small":
        return b
    if kind == "big":
        return int(b * 1.5)
    if kind == "boss":
        return int(b * 2)
    return 0


def to_real_shape(
    sim: dict[str, Any],
    *,
    seed: str | None = None,
    deck_name: str | None = None,
) -> dict[str, Any]:
    """Reshape a sim snapshot dict to match BalatroBot `gamestate` output.

    Fields the sim does not yet model are returned with explicit `None` so
    the diff tool can flag them instead of silently omitting.
    """
    # Prefer explicitly-passed values; fall back to sim-provided ones so sim
    # output can drive the diff on its own once those fields are plumbed.
    if seed is None:
        seed = sim.get("seed_str") or None
    if deck_name is None:
        deck_name = sim.get("deck_name") or None
    # Real-client convention is uppercase deck names ("RED", "BLUE", ...).
    deck_name = (deck_name or "").upper() or None

    stake_int = sim.get("stake")
    stake_name = sim.get("stake_name") or STAKE_BY_INT.get(stake_int, str(stake_int) if stake_int else None)

    # hand slice (sim calls it `available`)
    available = sim.get("available") or []
    hand_cards_out = [
        {
            "key": f"{SUIT_CODE.get(getattr(c, 'suit', c.get('suit') if isinstance(c, dict) else ''), '?')}"
                   f"_{RANK_CODE.get(getattr(c, 'rank', c.get('rank') if isinstance(c, dict) else ''), '?')}",
            "value": {
                "rank": getattr(c, 'rank', c.get('rank') if isinstance(c, dict) else None),
                "suit": getattr(c, 'suit', c.get('suit') if isinstance(c, dict) else None),
            },
            "label": "Base Card",
            "id": None,  # sim doesn't expose card IDs yet
        }
        for c in available
    ]

    sim_blind_states = sim.get("blind_states") or {}
    sim_tag_by_slot = {
        "small": sim.get("small_tag") or {},
        "big": sim.get("big_tag") or {},
        "boss": sim.get("boss_tag") or {},
    }
    blinds_out: dict[str, Any] = {}
    for kind_lc, kind_tc in (("small", "Small"), ("big", "Big"), ("boss", "Boss")):
        status = sim_blind_states.get(kind_tc)
        real_status = BLIND_STATE_TO_REAL.get(status, status)
        tag = sim_tag_by_slot.get(kind_lc) or {}
        # Boss slots never carry a tag in vanilla Balatro, matching the real
        # client's convention of empty strings rather than nulls.
        blinds_out[kind_lc] = {
            "status": real_status,
            "score": blind_target(sim.get("round", 1), sim.get("ante", 1), kind_lc),
            "name": {"small": "Small Blind", "big": "Big Blind", "boss": sim.get("blind_name", "Boss")}[kind_lc],
            "type": kind_lc.upper(),
            "effect": sim.get("boss_effect", "") if kind_lc == "boss" else "",
            "tag_name": tag.get("name", ""),
            "tag_effect": tag.get("description", ""),
        }

    plays_left = sim.get("plays", 0) or 0
    discards_left = sim.get("discards", 0) or 0
    # engine doesn't expose plays/discards used directly; infer from base 4/4:
    plays_base, discards_base = 4, 4
    hands_played = max(0, plays_base - plays_left)
    discards_used = max(0, discards_base - discards_left)

    return {
        "state": sim.get("lua_state") or SIM_STAGE_TO_LUA_STATE.get(sim.get("stage", ""), "?"),
        "seed": seed,                               # sim today: not plumbed through
        "deck": deck_name,                          # sim today: not plumbed through
        "stake": stake_name,
        "won": sim.get("won", False),
        "money": sim.get("money", 0),
        "ante_num": sim.get("ante"),
        "round_num": sim.get("round"),
        "round": {
            "hands_left": plays_left,
            "hands_played": hands_played,
            "discards_left": discards_left,
            "discards_used": discards_used,
            "chips": sim.get("score", 0),
            "reroll_cost": sim.get("shop_reroll_cost", 0),
        },
        "hand": {
            "cards": hand_cards_out,
            "count": len(available),
            "limit": 8,                             # sim hardcodes; not in snapshot
            "highlighted_limit": 5,
        },
        "jokers": {
            "cards": sim.get("jokers") or [],
            "count": len(sim.get("jokers") or []),
            "limit": 5,
            "highlighted_limit": 1,
        },
        "consumables": {
            "cards": sim.get("consumables") or [],
            "count": len(sim.get("consumables") or []),
            "limit": sim.get("consumable_slot_limit", 2),
            "highlighted_limit": 1,
        },
        "blinds": blinds_out,
        "used_vouchers": sim.get("owned_vouchers") or [],
        "vouchers": {
            "cards": [sim.get("shop_voucher")] if sim.get("shop_voucher") else [],
            "count": 1 if sim.get("shop_voucher") else 0,
            "limit": 1,
            "highlighted_limit": 1,
        },
        "shop": {
            # real client uses a single `shop.cards` list mixing jokers and
            # consumables; sim keeps them segregated. We unify here so the
            # diff can compare apples-to-apples.
            "cards": (sim.get("shop_jokers") or []) + (sim.get("shop_consumables") or []),
            "count": len(sim.get("shop_jokers") or []) + len(sim.get("shop_consumables") or []),
            "limit": 2,
            "highlighted_limit": 1,
        },
        "cards": {
            "cards": sim.get("deck") or [],
            "count": len(sim.get("deck") or []),
            "limit": None,
            "highlighted_limit": None,
        },
        "packs": {
            "cards": [sim.get("open_pack")] if sim.get("open_pack") else [],
            "count": 1 if sim.get("open_pack") else 0,
            "limit": None,
            "highlighted_limit": None,
        },
        # Engine's `hand_stats` matches the real-client `hands` shape field-
        # for-field (sans `example`, which is not modeled yet). Keys are the
        # human display name ("Pair", "Flush", ...). Fall back to an empty dict
        # for snapshots produced by older sim builds that don't emit it.
        "hands": sim.get("hand_stats") or {},
    }


# --- diff primitives ------------------------------------------------------

ALIGNED = "aligned"
MISSING_IN_SIM = "missing_in_sim"
MISSING_IN_REAL = "missing_in_real"
VALUE_MISMATCH = "value_mismatch"
SHAPE_MISMATCH = "shape_mismatch"


def _compare(real: Any, sim: Any, path: str, rows: list[dict[str, Any]]) -> None:
    if real is None and sim is None:
        rows.append({"path": path, "status": ALIGNED, "real": None, "sim": None})
        return
    if real is None:
        rows.append({"path": path, "status": MISSING_IN_REAL, "real": None, "sim": _preview(sim)})
        return
    if sim is None:
        rows.append({"path": path, "status": MISSING_IN_SIM, "real": _preview(real), "sim": None})
        return
    if type(real) is not type(sim):
        rows.append({
            "path": path,
            "status": SHAPE_MISMATCH,
            "real": f"{type(real).__name__}:{_preview(real)}",
            "sim": f"{type(sim).__name__}:{_preview(sim)}",
        })
        return
    if isinstance(real, dict):
        for k in sorted(set(real) | set(sim)):
            _compare(real.get(k), sim.get(k), f"{path}.{k}" if path else k, rows)
        return
    if isinstance(real, list):
        # compare lengths + first element type; deep element diff out of scope
        if len(real) != len(sim):
            rows.append({
                "path": path,
                "status": VALUE_MISMATCH,
                "real": f"list[{len(real)}]",
                "sim": f"list[{len(sim)}]",
            })
        else:
            rows.append({
                "path": path,
                "status": ALIGNED,
                "real": f"list[{len(real)}]",
                "sim": f"list[{len(sim)}]",
            })
        return
    # scalar
    if real == sim:
        rows.append({"path": path, "status": ALIGNED, "real": _preview(real), "sim": _preview(sim)})
    else:
        rows.append({"path": path, "status": VALUE_MISMATCH, "real": _preview(real), "sim": _preview(sim)})


def _preview(v: Any, maxlen: int = 60) -> Any:
    if isinstance(v, (dict, list)):
        s = str(v)
    else:
        s = repr(v)
    return s if len(s) <= maxlen else s[: maxlen - 1] + "…"


def diff_shapes(real: dict[str, Any], normalized_sim: dict[str, Any]) -> list[dict[str, Any]]:
    """Flat diff between real and sim (after normalization).

    Returns rows: [{"path", "status", "real", "sim"}].
    """
    rows: list[dict[str, Any]] = []
    _compare(real, normalized_sim, "", rows)
    return rows
