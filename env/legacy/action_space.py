from __future__ import annotations

ACTION_DIM = 86

SELECT_CARD_START = 0
SELECT_CARD_COUNT = 8
PLAY_INDEX = 8
DISCARD_INDEX = 9
SELECT_BLIND_START = 10
SELECT_BLIND_COUNT = 3
CASHOUT_INDEX = 13
BUY_JOKER_START = 14
BUY_JOKER_COUNT = 10
MOVE_LEFT_START = 24
MOVE_LEFT_COUNT = 23
MOVE_RIGHT_START = 47
MOVE_RIGHT_COUNT = 23
NEXT_ROUND_INDEX = 70
USE_CONSUMABLE_START = 71
USE_CONSUMABLE_COUNT = 8
REROLL_SHOP_INDEX = 79
SELL_JOKER_START = 80
SELL_JOKER_COUNT = 5
SKIP_BLIND_INDEX = 85


def action_name(index: int) -> str:
    if SELECT_CARD_START <= index < SELECT_CARD_START + SELECT_CARD_COUNT:
        return f"select_card_{index - SELECT_CARD_START}"
    if index == PLAY_INDEX:
        return "play"
    if index == DISCARD_INDEX:
        return "discard"
    if SELECT_BLIND_START <= index < SELECT_BLIND_START + SELECT_BLIND_COUNT:
        return f"select_blind_{index - SELECT_BLIND_START}"
    if index == CASHOUT_INDEX:
        return "cashout"
    if BUY_JOKER_START <= index < BUY_JOKER_START + BUY_JOKER_COUNT:
        return f"buy_shop_item_{index - BUY_JOKER_START}"
    if MOVE_LEFT_START <= index < MOVE_LEFT_START + MOVE_LEFT_COUNT:
        return f"move_left_{index - MOVE_LEFT_START}"
    if MOVE_RIGHT_START <= index < MOVE_RIGHT_START + MOVE_RIGHT_COUNT:
        return f"move_right_{index - MOVE_RIGHT_START}"
    if index == NEXT_ROUND_INDEX:
        return "next_round"
    if USE_CONSUMABLE_START <= index < USE_CONSUMABLE_START + USE_CONSUMABLE_COUNT:
        return f"use_consumable_{index - USE_CONSUMABLE_START}"
    if index == REROLL_SHOP_INDEX:
        return "reroll_shop"
    if SELL_JOKER_START <= index < SELL_JOKER_START + SELL_JOKER_COUNT:
        return f"sell_joker_{index - SELL_JOKER_START}"
    if index == SKIP_BLIND_INDEX:
        return "skip_blind"
    return f"unknown_{index}"
