from __future__ import annotations

import itertools
from dataclasses import dataclass
from typing import Any

import numpy as np

from env.action_space import (
    BUY_JOKER_COUNT,
    BUY_JOKER_START,
    CASHOUT_INDEX,
    DISCARD_INDEX,
    NEXT_ROUND_INDEX,
    PLAY_INDEX,
    REROLL_SHOP_INDEX,
    SELECT_BLIND_COUNT,
    SELECT_BLIND_START,
    SELECT_CARD_COUNT,
    SELECT_CARD_START,
)
from env.state_encoder import (
    ACTION_MASK_SIZE,
    HAND_CARD_FEATURES,
    HAND_SLOTS,
    NUM_SCALARS,
    NUM_STAGES,
    OFF_HAND_CARDS,
    OFF_SCALARS,
    OFF_STAGE,
    classify_hand_direct,
)


@dataclass
class _Card:
    slot: int
    rank: int
    suit: int
    selected: float
    chip_value: float


class GreedyAgent:
    """Heuristic baseline that reads structured data from flat observation."""

    def __init__(self, play_threshold: int = 0) -> None:
        self.play_threshold = play_threshold

    def _stage(self, obs: np.ndarray) -> int:
        stage = obs[OFF_STAGE : OFF_STAGE + NUM_STAGES]
        if not np.any(stage > 0.5):
            return NUM_STAGES - 1
        return int(np.argmax(stage))

    def _extract_cards(self, obs: np.ndarray) -> list[_Card]:
        cards_raw = obs[OFF_HAND_CARDS : OFF_HAND_CARDS + HAND_SLOTS * HAND_CARD_FEATURES]
        cards = cards_raw.reshape(HAND_SLOTS, HAND_CARD_FEATURES)

        result: list[_Card] = []
        for slot in range(HAND_SLOTS):
            row = cards[slot]
            if np.sum(np.abs(row)) < 1e-6:
                continue
            rank = int(np.argmax(row[:13]))
            suit = int(np.argmax(row[13:17]))
            result.append(
                _Card(
                    slot=slot,
                    rank=rank,
                    suit=suit,
                    selected=float(row[17]),
                    chip_value=float(row[18]),
                )
            )
        return result

    def _best_hand_type(self, cards: list[_Card]) -> int:
        if not cards:
            return 0
        best = 0
        max_size = min(5, len(cards))
        for size in range(1, max_size + 1):
            for combo in itertools.combinations(cards, size):
                idx = classify_hand_direct([c.rank for c in combo], [c.suit for c in combo])
                if idx > best:
                    best = idx
        return best

    def _blind_action(self, obs: np.ndarray, action_mask: np.ndarray) -> int:
        cards = self._extract_cards(obs)
        best_type = self._best_hand_type(cards)
        selected_cards = [c for c in cards if c.selected > 0.5]
        unselected_cards = [c for c in cards if c.selected <= 0.5]

        plays = float(obs[OFF_SCALARS + 3]) * 10.0
        discards = float(obs[OFF_SCALARS + 4]) * 10.0

        if action_mask[PLAY_INDEX] and (best_type >= self.play_threshold or discards <= 0 or plays <= 1):
            return PLAY_INDEX

        # Select up to 5 high-value cards before playing to avoid weak single-card plays.
        if action_mask[PLAY_INDEX] and len(selected_cards) < min(5, len(cards)) and unselected_cards:
            best = max(unselected_cards, key=lambda c: c.chip_value)
            candidate = SELECT_CARD_START + best.slot
            if candidate < ACTION_MASK_SIZE and action_mask[candidate]:
                return candidate

        if action_mask[DISCARD_INDEX]:
            return DISCARD_INDEX

        for slot in range(SELECT_CARD_START, SELECT_CARD_START + SELECT_CARD_COUNT):
            if slot < ACTION_MASK_SIZE and action_mask[slot]:
                return slot

        valid = np.flatnonzero(action_mask)
        return int(valid[0]) if valid.size else 0

    def _shop_action(self, action_mask: np.ndarray) -> int:
        for i in range(BUY_JOKER_START, BUY_JOKER_START + BUY_JOKER_COUNT):
            if i < action_mask.size and action_mask[i]:
                return i
        if REROLL_SHOP_INDEX < action_mask.size and action_mask[REROLL_SHOP_INDEX]:
            return REROLL_SHOP_INDEX
        if NEXT_ROUND_INDEX < action_mask.size and action_mask[NEXT_ROUND_INDEX]:
            return NEXT_ROUND_INDEX
        valid = np.flatnonzero(action_mask)
        return int(valid[0]) if valid.size else 0

    def act(self, obs: np.ndarray, info: dict[str, Any] | None = None, action_mask: np.ndarray | None = None) -> int:
        del info
        if action_mask is None:
            action_mask = obs[:ACTION_MASK_SIZE] > 0.5

        stage_idx = self._stage(obs)

        # Stage index mapping from state_encoder.STAGES
        if stage_idx == 0:  # PreBlind
            for i in range(SELECT_BLIND_START, SELECT_BLIND_START + SELECT_BLIND_COUNT):
                if i < action_mask.size and action_mask[i]:
                    return i
        if stage_idx == 1:  # Blind
            return self._blind_action(obs, action_mask)
        if stage_idx == 2:  # PostBlind
            if CASHOUT_INDEX < action_mask.size and action_mask[CASHOUT_INDEX]:
                return CASHOUT_INDEX
        if stage_idx == 3:  # Shop
            return self._shop_action(action_mask)

        valid = np.flatnonzero(action_mask)
        return int(valid[0]) if valid.size else 0
