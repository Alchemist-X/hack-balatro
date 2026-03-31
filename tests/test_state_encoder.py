from __future__ import annotations

import numpy as np

from env.action_space import ACTION_DIM
from env.state_encoder import (
    BOSS_DIM,
    CONSUMABLE_DIM,
    HAND_CARD_FEATURES,
    HAND_SLOTS,
    OBS_DIM,
    OFF_BOSS,
    OFF_CONSUMABLE,
    OFF_HAND_CARDS,
    OFF_VOUCHER,
    VOUCHER_DIM,
    encode_pylatro_state,
    unpack_obs_to_structured,
)


class Card:
    def __init__(self, card_id: int, rank_index: int, suit_index: int, chip_value: int) -> None:
        self.card_id = card_id
        self.rank_index = rank_index
        self.suit_index = suit_index
        self.chip_value = chip_value


class State:
    def __init__(self) -> None:
        self.stage = "Stage_Blind"
        self.round = 1
        self.score = 20
        self.required_score = 200
        self.plays = 3
        self.discards = 2
        self.money = 5
        self.ante = 1
        self.reward = 3
        self.boss_effect = "TheClub"
        self.available = [Card(1, 0, 0, 2), Card(2, 0, 1, 2), Card(3, 4, 2, 6)]
        self.selected = [self.available[0], self.available[1]]
        self.deck = [Card(4, 5, 3, 7)]
        self.discarded = []
        self.jokers = []
        self.shop_jokers = []


def test_encode_dimensions_and_offsets() -> None:
    state = State()
    mask = np.zeros(ACTION_DIM, dtype=np.float32)
    mask[:10] = 1.0

    obs = encode_pylatro_state(state, mask)
    assert obs.shape == (OBS_DIM,)

    hand_block = obs[OFF_HAND_CARDS : OFF_HAND_CARDS + HAND_SLOTS * HAND_CARD_FEATURES].reshape(
        HAND_SLOTS,
        HAND_CARD_FEATURES,
    )
    assert hand_block[0, 0] == 1.0
    assert hand_block[1, 0] == 1.0
    assert hand_block[2, 4] == 1.0

    # Boss one-hot exists
    assert np.sum(obs[OFF_BOSS : OFF_BOSS + BOSS_DIM]) == 1.0


def test_unpack_obs_shapes() -> None:
    state = State()
    mask = np.ones(ACTION_DIM, dtype=np.float32)
    obs = encode_pylatro_state(state, mask)
    s = unpack_obs_to_structured(obs)

    assert s["card_features"].shape == (1, HAND_SLOTS, HAND_CARD_FEATURES)
    assert s["card_mask"].shape == (1, HAND_SLOTS)
    assert s["joker_ids"].shape == (1, 5)
    assert s["joker_mask"].shape == (1, 5)
    # global_features = stages(7) + scalars(18) + selected_hand(12) + best_hand(12)
    #   + deck(52) + discarded(52) + joker_shop(10) + boss(28) + consumable(2) + voucher(10)
    expected_global = 7 + 18 + 12 + 12 + 52 + 52 + 10 + 28 + 2 + 10
    assert s["global_features"].shape == (1, expected_global)
    assert s["action_mask"].shape == (1, ACTION_DIM)
