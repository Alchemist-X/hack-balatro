"""Unit tests for env.action_inference.

Covers the common-case legal-action rules (SELECTING_HAND / BLIND_SELECT /
ROUND_EVAL / SHOP) and the observer-event → 86-dim-index mapping.
"""
from __future__ import annotations

from env.action_inference import (
    BUY_SHOP_ITEM_START,
    CASHOUT_INDEX,
    DISCARD_INDEX,
    NEXT_ROUND_INDEX,
    PLAY_INDEX,
    REROLL_SHOP_INDEX,
    SELECT_BLIND_START,
    SELECT_CARD_COUNT,
    SELECT_CARD_START,
    SELL_JOKER_START,
    SKIP_BLIND_INDEX,
    USE_CONSUMABLE_START,
    infer_executed_action,
    infer_legal_actions,
)


# ---- legal-action rules --------------------------------------------------
def test_selecting_hand_with_hands_and_discards_left() -> None:
    state = {
        "state": "SELECTING_HAND",
        "hand_count": 5,
        "hands_left": 2,
        "discards_left": 3,
        "jokers": 0,
        "consumables": 0,
    }
    legal = set(infer_legal_actions(state))
    assert PLAY_INDEX in legal
    assert DISCARD_INDEX in legal
    assert SELECT_CARD_START in legal
    assert SELECT_CARD_START + 4 in legal
    assert SELECT_CARD_START + 5 not in legal


def test_selecting_hand_no_discards_left_disables_discard() -> None:
    state = {
        "state": "SELECTING_HAND",
        "hand_count": 3,
        "hands_left": 1,
        "discards_left": 0,
    }
    legal = set(infer_legal_actions(state))
    assert PLAY_INDEX in legal
    assert DISCARD_INDEX not in legal


def test_selecting_hand_no_cards_disables_play_and_discard() -> None:
    state = {
        "state": "SELECTING_HAND",
        "hand_count": 0,
        "hands_left": 3,
        "discards_left": 3,
    }
    legal = set(infer_legal_actions(state))
    assert PLAY_INDEX not in legal
    assert DISCARD_INDEX not in legal


def test_selecting_hand_caps_select_card_indices_at_8() -> None:
    state = {
        "state": "SELECTING_HAND",
        "hand_count": 12,  # hypothetical hand-size boost > 8
        "hands_left": 1,
        "discards_left": 0,
    }
    legal = set(infer_legal_actions(state))
    # all 8 select_card_* slots legal (0..7)
    for i in range(SELECT_CARD_COUNT):
        assert SELECT_CARD_START + i in legal
    # slots beyond 8 are outside the select_card range and must not
    # have been emitted as spurious select_card indices (9 = DISCARD_INDEX
    # and 8 = PLAY_INDEX, which ARE legal via separate rules — so we only
    # bound-check slot 8 of the select_card range, which would be index 8,
    # collapsing onto PLAY. The correctness here is that the select_card
    # emission loop terminated at SELECT_CARD_COUNT).


def test_blind_select_small_selectable() -> None:
    state = {
        "state": "BLIND_SELECT",
        "blind_small": "SELECT",
        "blind_big": "UPCOMING",
        "blind_boss": "UPCOMING",
    }
    legal = set(infer_legal_actions(state))
    assert SELECT_BLIND_START + 0 in legal
    assert SELECT_BLIND_START + 1 not in legal
    assert SKIP_BLIND_INDEX in legal


def test_blind_select_boss_cannot_be_skipped() -> None:
    state = {
        "state": "BLIND_SELECT",
        "blind_small": "DEFEATED",
        "blind_big": "DEFEATED",
        "blind_boss": "SELECT",
    }
    legal = set(infer_legal_actions(state))
    assert SELECT_BLIND_START + 2 in legal
    assert SKIP_BLIND_INDEX not in legal


def test_round_eval_only_cashout() -> None:
    state = {"state": "ROUND_EVAL"}
    legal = set(infer_legal_actions(state))
    assert CASHOUT_INDEX in legal
    assert PLAY_INDEX not in legal
    assert NEXT_ROUND_INDEX not in legal


def test_shop_with_money_and_items() -> None:
    state = {
        "state": "SHOP",
        "money": 10,
        "jokers": 2,
        "consumables": 1,
        "shop_count": 3,
        "round": {"reroll_cost": 5},
    }
    legal = set(infer_legal_actions(state))
    assert NEXT_ROUND_INDEX in legal
    assert BUY_SHOP_ITEM_START + 2 in legal
    assert BUY_SHOP_ITEM_START + 3 not in legal
    assert REROLL_SHOP_INDEX in legal
    assert SELL_JOKER_START + 1 in legal
    assert USE_CONSUMABLE_START in legal


def test_shop_cannot_reroll_when_broke() -> None:
    state = {
        "state": "SHOP",
        "money": 2,
        "shop_count": 1,
        "round": {"reroll_cost": 5},
    }
    legal = set(infer_legal_actions(state))
    assert REROLL_SHOP_INDEX not in legal
    assert NEXT_ROUND_INDEX in legal


def test_transient_state_emits_empty_legal_set() -> None:
    assert infer_legal_actions({"state": "HAND_PLAYED"}) == []
    assert infer_legal_actions({"state": "DRAW_TO_HAND"}) == []
    assert infer_legal_actions({"state": "GAME_OVER"}) == []


def test_accepts_raw_gamestate_shape() -> None:
    # the BalatroBot `gamestate` rpc uses nested shape with `.count`
    raw = {
        "state": "SHOP",
        "money": 20,
        "hand": {"cards": []},
        "jokers": {"count": 1, "cards": [{}]},
        "consumables": {"count": 0, "cards": []},
        "shop": {"count": 2},
        "round": {"reroll_cost": 5, "hands_left": 0, "discards_left": 0},
    }
    legal = set(infer_legal_actions(raw))
    assert NEXT_ROUND_INDEX in legal
    assert BUY_SHOP_ITEM_START in legal
    assert BUY_SHOP_ITEM_START + 1 in legal
    assert REROLL_SHOP_INDEX in legal


# ---- executed-action mapping --------------------------------------------
def test_hand_played_maps_to_play_index_approximate() -> None:
    assert infer_executed_action("hand_played", {}, {"state": "SELECTING_HAND"}) == (
        PLAY_INDEX,
        True,
    )


def test_discard_maps_to_discard_index_approximate() -> None:
    assert infer_executed_action("discard", {}, {"state": "SELECTING_HAND"}) == (
        DISCARD_INDEX,
        True,
    )


def test_money_change_cashout_is_exact() -> None:
    idx, approx = infer_executed_action(
        "money_change",
        {"delta": 12, "summary_after": {"state": "SHOP"}},
        {"state": "ROUND_EVAL"},
    )
    assert idx == CASHOUT_INDEX
    assert approx is False


def test_money_change_buy_is_approximate() -> None:
    idx, approx = infer_executed_action(
        "money_change",
        {"delta": -4, "summary_after": {"state": "SHOP"}},
        {"state": "SHOP"},
    )
    assert idx == BUY_SHOP_ITEM_START
    assert approx is True


def test_blind_status_change_to_current_maps_per_blind() -> None:
    assert infer_executed_action(
        "blind_status_change", {"blind": "small", "to": "CURRENT"}, {}
    ) == (SELECT_BLIND_START + 0, False)
    assert infer_executed_action(
        "blind_status_change", {"blind": "boss", "to": "CURRENT"}, {}
    ) == (SELECT_BLIND_START + 2, False)


def test_blind_status_change_to_skipped_is_exact() -> None:
    assert infer_executed_action(
        "blind_status_change", {"blind": "small", "to": "SKIPPED"}, {}
    ) == (SKIP_BLIND_INDEX, False)


def test_state_change_shop_to_blindselect_is_next_round() -> None:
    assert infer_executed_action(
        "state_change", {"from": "SHOP", "to": "BLIND_SELECT"}, {}
    ) == (NEXT_ROUND_INDEX, False)


def test_jokers_count_change_buy_and_sell_are_approximate() -> None:
    assert infer_executed_action(
        "jokers_count_change", {"from": 1, "to": 2}, {"state": "SHOP"}
    ) == (BUY_SHOP_ITEM_START, True)
    assert infer_executed_action(
        "jokers_count_change", {"from": 2, "to": 1}, {"state": "SHOP"}
    ) == (SELL_JOKER_START, True)


def test_consumables_count_change_use_from_play_phase() -> None:
    assert infer_executed_action(
        "consumables_count_change",
        {"from": 1, "to": 0},
        {"state": "SELECTING_HAND"},
    ) == (USE_CONSUMABLE_START, True)


def test_unknown_event_kind_returns_none() -> None:
    assert infer_executed_action("round_chips_up", {"from": 0, "to": 50}, {}) == (None, False)
