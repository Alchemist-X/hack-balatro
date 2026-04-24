"""Rule-based legal-action and executed-action inference for observer data.

The BalatroBot observer captures `gamestate` snapshots and inferred events,
but does not record the 86-dim action index that a learning agent would
emit. This module reconstructs both:

  - `infer_legal_actions(gamestate)`: returns the set of indices that would
    be legal given the current snapshot (approximate — captures common case;
    boss/voucher-specific restrictions may be missed).
  - `infer_executed_action(event_kind, event_payload, state_before)`: maps
    an observer event (e.g. `hand_played`, `discard`, `money_change`) to a
    single 86-dim index. Composite events (e.g. select-then-play collapsed
    into one `hand_played`) are flagged as `approximate=True`.

The 86-action taxonomy is replicated here (NOT imported from `env.legacy`)
so that this module is the canonical source going forward.

Supported input shapes
----------------------
Both the observer's `summary` dict (flat, uses `hand_count`/`jokers` as int
count) and a raw BalatroBot `gamestate` (nested, uses `hand.count` /
`jokers.count`) are accepted. `_get_count(...)` normalises access.
"""
from __future__ import annotations

from typing import Any


# ---- action taxonomy (authoritative; replicate, do not import legacy) ----
ACTION_DIM = 86

SELECT_CARD_START = 0        # 0..7
SELECT_CARD_COUNT = 8
PLAY_INDEX = 8
DISCARD_INDEX = 9
SELECT_BLIND_START = 10      # 10..12 (small, big, boss)
SELECT_BLIND_COUNT = 3
CASHOUT_INDEX = 13
BUY_SHOP_ITEM_START = 14     # 14..23
BUY_SHOP_ITEM_COUNT = 10
MOVE_LEFT_START = 24         # 24..46
MOVE_LEFT_COUNT = 23
MOVE_RIGHT_START = 47        # 47..69
MOVE_RIGHT_COUNT = 23
NEXT_ROUND_INDEX = 70
USE_CONSUMABLE_START = 71    # 71..78
USE_CONSUMABLE_COUNT = 8
REROLL_SHOP_INDEX = 79
SELL_JOKER_START = 80        # 80..84
SELL_JOKER_COUNT = 5
SKIP_BLIND_INDEX = 85


_BLIND_ORDER = ("small", "big", "boss")


# ---- state helpers -------------------------------------------------------
def _get_count(state: dict[str, Any], collection: str) -> int:
    """Return a count for `hand`, `jokers`, `consumables`, `shop`.

    Works for both the observer `summary` dict (flat `hand_count`, `jokers`
    as int) and a raw BalatroBot `gamestate` (nested `.count`).
    Returns 0 if unavailable.
    """
    if not isinstance(state, dict):
        return 0
    # summary shape
    if collection == "hand":
        v = state.get("hand_count")
        if isinstance(v, int):
            return v
        # from hand_cards list
        hc = state.get("hand_cards")
        if isinstance(hc, list):
            return len(hc)
    if collection == "jokers":
        v = state.get("jokers")
        if isinstance(v, int):
            return v
    if collection == "consumables":
        v = state.get("consumables")
        if isinstance(v, int):
            return v
    if collection == "shop":
        v = state.get("shop_count")
        if isinstance(v, int):
            return v
    # raw gamestate shape
    nested = state.get(collection)
    if isinstance(nested, dict):
        c = nested.get("count")
        if isinstance(c, int):
            return c
        cards = nested.get("cards")
        if isinstance(cards, list):
            return len(cards)
    return 0


def _blind_status(state: dict[str, Any], which: str) -> str | None:
    """Read a blind status (SELECT/CURRENT/UPCOMING/DEFEATED/SKIPPED)."""
    if not isinstance(state, dict):
        return None
    flat = state.get(f"blind_{which}")
    if isinstance(flat, str):
        return flat
    blinds = state.get("blinds")
    if isinstance(blinds, dict):
        b = blinds.get(which)
        if isinstance(b, dict):
            s = b.get("status")
            if isinstance(s, str):
                return s
    return None


def _money(state: dict[str, Any]) -> int | float | None:
    v = state.get("money") if isinstance(state, dict) else None
    return v if isinstance(v, (int, float)) else None


def _reroll_cost(state: dict[str, Any]) -> int | None:
    if not isinstance(state, dict):
        return None
    # summary shape surfaces reroll_cost at top level
    rc_flat = state.get("reroll_cost")
    if isinstance(rc_flat, int):
        return rc_flat
    r = state.get("round")
    if isinstance(r, dict):
        rc = r.get("reroll_cost")
        if isinstance(rc, int):
            return rc
    return None


# ---- legal-action inference ---------------------------------------------
def infer_legal_actions(gamestate: dict[str, Any]) -> list[int]:
    """Return the sorted list of 86-dim action indices that are legal now.

    Approximations (not reflected here):
      - Voucher effects changing shop size (Overstock) or joker limit
      - Boss blinds that force specific card exclusions (e.g. The Needle)
      - Seal / enhancement constraints on playability
      - Hand-size reducing effects

    These are out-of-scope for the observer's retrofit. Downstream can
    intersect with a voucher-aware mask if available.
    """
    if not isinstance(gamestate, dict):
        return []
    state = gamestate.get("state")
    legal: set[int] = set()

    hand_count = _get_count(gamestate, "hand")
    jokers_count = _get_count(gamestate, "jokers")
    consumables_count = _get_count(gamestate, "consumables")
    shop_count = _get_count(gamestate, "shop")

    hands_left = gamestate.get("hands_left")
    if hands_left is None:
        r = gamestate.get("round")
        if isinstance(r, dict):
            hands_left = r.get("hands_left")
    discards_left = gamestate.get("discards_left")
    if discards_left is None:
        r = gamestate.get("round")
        if isinstance(r, dict):
            discards_left = r.get("discards_left")

    if state == "SELECTING_HAND":
        # toggle any of the current hand slots
        for i in range(min(hand_count, SELECT_CARD_COUNT)):
            legal.add(SELECT_CARD_START + i)
        if isinstance(hands_left, int) and hands_left > 0 and hand_count > 0:
            legal.add(PLAY_INDEX)
        if isinstance(discards_left, int) and discards_left > 0 and hand_count > 0:
            legal.add(DISCARD_INDEX)
        # use_consumable_* usable mid-hand
        for i in range(min(consumables_count, USE_CONSUMABLE_COUNT)):
            legal.add(USE_CONSUMABLE_START + i)

    elif state == "BLIND_SELECT":
        # only the blind whose status is SELECT is choosable (usually one
        # of small, big, boss depending on round progression).
        selectable = [
            i for i, which in enumerate(_BLIND_ORDER)
            if _blind_status(gamestate, which) == "SELECT"
        ]
        for i in selectable:
            if i < SELECT_BLIND_COUNT:
                legal.add(SELECT_BLIND_START + i)
        # skip only for small/big, never for boss
        if any(i < 2 for i in selectable):
            legal.add(SKIP_BLIND_INDEX)

    elif state == "ROUND_EVAL":
        legal.add(CASHOUT_INDEX)

    elif state == "SHOP":
        legal.add(NEXT_ROUND_INDEX)
        for i in range(min(shop_count, BUY_SHOP_ITEM_COUNT)):
            legal.add(BUY_SHOP_ITEM_START + i)
        # reroll iff enough money
        m = _money(gamestate)
        rc = _reroll_cost(gamestate)
        if isinstance(m, (int, float)) and isinstance(rc, int) and m >= rc:
            legal.add(REROLL_SHOP_INDEX)
        # consumables usable in shop
        for i in range(min(consumables_count, USE_CONSUMABLE_COUNT)):
            legal.add(USE_CONSUMABLE_START + i)
        # sell owned jokers
        for i in range(min(jokers_count, SELL_JOKER_COUNT)):
            legal.add(SELL_JOKER_START + i)

    # transient / non-decision phases (HAND_PLAYED, DRAW_TO_HAND,
    # PLAY_TAROT, SMODS_BOOSTER_OPENED, GAME_OVER, MENU) emit no legal
    # actions — the agent does not decide during these. Callers should
    # treat empty-list as "no-op tick" rather than "game error".

    return sorted(legal)


# ---- executed-action inference -------------------------------------------
def infer_executed_action(
    event_kind: str,
    event_payload: dict[str, Any],
    state_before: dict[str, Any] | None = None,
) -> tuple[int | None, bool]:
    """Map an observer event to a 86-dim action index.

    Returns (index, approximate). `approximate=True` means the true action
    is a composite (e.g. select_card * N + play collapsed into one
    hand_played event) or the event is ambiguous (e.g. money_change in
    SHOP with no inventory delta could be any buy slot).

    `state_before` is consulted only when needed to disambiguate
    (e.g. consumables_count_change during SELECTING_HAND → use_consumable
    vs SHOP → buy).
    """
    if not isinstance(event_payload, dict):
        event_payload = {}
    state_before = state_before if isinstance(state_before, dict) else {}

    if event_kind == "hand_played":
        # composite: human selected N cards then pressed PLAY.
        return PLAY_INDEX, True

    if event_kind == "discard":
        return DISCARD_INDEX, True

    if event_kind == "jokers_count_change":
        frm = event_payload.get("from") or 0
        to = event_payload.get("to") or 0
        if to > frm:
            # unknown shop slot — assume slot 0
            return BUY_SHOP_ITEM_START, True
        if to < frm:
            # unknown sell slot — assume slot 0
            return SELL_JOKER_START, True
        return None, False

    if event_kind == "consumables_count_change":
        frm = event_payload.get("from") or 0
        to = event_payload.get("to") or 0
        state = state_before.get("state")
        if to > frm:
            # bought a consumable (shop); unknown slot — assume slot 0
            return BUY_SHOP_ITEM_START, True
        if to < frm:
            # consumed it (use_consumable); unknown slot — assume slot 0
            # Valid in SELECTING_HAND and SHOP; also via pack opens.
            if state in {"SELECTING_HAND", "SHOP", "PLAY_TAROT",
                         "SMODS_BOOSTER_OPENED", "BOOSTER_OPENED"}:
                return USE_CONSUMABLE_START, True
            return USE_CONSUMABLE_START, True
        return None, False

    if event_kind == "money_change":
        delta = event_payload.get("delta", 0) or 0
        state = state_before.get("state")
        # Event is emitted with summary_after already populated; caller may
        # also pass the new state via payload["summary_after"] but we use
        # state_before for provenance attribution.
        after_state = None
        sa = event_payload.get("summary_after")
        if isinstance(sa, dict):
            after_state = sa.get("state")
        # cash_out: positive delta transitioning to SHOP
        if delta > 0 and after_state == "SHOP":
            return CASHOUT_INDEX, False
        # buy: negative delta in SHOP (couldn't pin to slot)
        if delta < 0 and after_state == "SHOP":
            return BUY_SHOP_ITEM_START, True
        # buying a pack that opens
        if delta < 0 and after_state in {"SMODS_BOOSTER_OPENED", "BOOSTER_OPENED"}:
            return BUY_SHOP_ITEM_START, True
        return None, False

    if event_kind == "blind_status_change":
        to = event_payload.get("to")
        which = event_payload.get("blind")
        if to == "SKIPPED":
            return SKIP_BLIND_INDEX, False
        if to == "CURRENT":
            try:
                offset = _BLIND_ORDER.index(which)
            except ValueError:
                return None, False
            return SELECT_BLIND_START + offset, False
        return None, False

    if event_kind == "state_change":
        frm = event_payload.get("from")
        to = event_payload.get("to")
        if frm == "SHOP" and to == "BLIND_SELECT":
            return NEXT_ROUND_INDEX, False
        return None, False

    return None, False


# ---- self-test -----------------------------------------------------------
if __name__ == "__main__":
    # smoke-test inference against a few canned states
    shop_state = {
        "state": "SHOP",
        "money": 10,
        "hand_count": 0,
        "jokers": 2,
        "consumables": 1,
        "shop": {"count": 2},
        "round": {"reroll_cost": 5},
    }
    legal = infer_legal_actions(shop_state)
    assert NEXT_ROUND_INDEX in legal
    assert BUY_SHOP_ITEM_START in legal
    assert BUY_SHOP_ITEM_START + 1 in legal
    assert REROLL_SHOP_INDEX in legal
    assert USE_CONSUMABLE_START in legal
    assert SELL_JOKER_START in legal
    assert SELL_JOKER_START + 1 in legal

    play_state = {
        "state": "SELECTING_HAND",
        "hand_count": 5,
        "hands_left": 3,
        "discards_left": 2,
        "jokers": 1,
        "consumables": 0,
    }
    legal = infer_legal_actions(play_state)
    assert PLAY_INDEX in legal
    assert DISCARD_INDEX in legal
    assert SELECT_CARD_START in legal
    assert SELECT_CARD_START + 4 in legal
    assert SELECT_CARD_START + 5 not in legal

    blind_state = {
        "state": "BLIND_SELECT",
        "blind_small": "SELECT",
        "blind_big": "UPCOMING",
        "blind_boss": "UPCOMING",
    }
    legal = infer_legal_actions(blind_state)
    assert SELECT_BLIND_START in legal
    assert SKIP_BLIND_INDEX in legal

    # inferred executed actions
    assert infer_executed_action("hand_played", {}, {"state": "SELECTING_HAND"}) == (PLAY_INDEX, True)
    assert infer_executed_action("discard", {}, {}) == (DISCARD_INDEX, True)
    assert infer_executed_action("state_change", {"from": "SHOP", "to": "BLIND_SELECT"}, {}) == (
        NEXT_ROUND_INDEX,
        False,
    )
    assert infer_executed_action("blind_status_change", {"blind": "big", "to": "SKIPPED"}, {}) == (
        SKIP_BLIND_INDEX,
        False,
    )
    assert infer_executed_action(
        "money_change", {"delta": 12, "summary_after": {"state": "SHOP"}}, {"state": "ROUND_EVAL"}
    ) == (CASHOUT_INDEX, False)

    print("env.action_inference: all smoke tests passed")
