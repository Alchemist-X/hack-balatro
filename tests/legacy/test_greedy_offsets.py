from __future__ import annotations

import numpy as np

from agents.greedy_agent import GreedyAgent
from env.legacy.action_space import ACTION_DIM, DISCARD_INDEX, PLAY_INDEX
from env.legacy.state_encoder import (
    HAND_CARD_FEATURES,
    HAND_SLOTS,
    NUM_SCALARS,
    NUM_STAGES,
    OBS_DIM,
    OFF_HAND_CARDS,
    OFF_SCALARS,
    OFF_STAGE,
)


def _build_obs_with_pair() -> np.ndarray:
    obs = np.zeros(OBS_DIM, dtype=np.float32)

    # Stage_Blind one-hot index=1
    obs[OFF_STAGE + 1] = 1.0

    # scalars
    scalars = np.zeros(NUM_SCALARS, dtype=np.float32)
    scalars[3] = 0.3  # plays=3
    scalars[4] = 0.2  # discards=2
    obs[OFF_SCALARS : OFF_SCALARS + NUM_SCALARS] = scalars

    # first two cards same rank -> pair
    card_block = np.zeros((HAND_SLOTS, HAND_CARD_FEATURES), dtype=np.float32)
    card_block[0, 3] = 1.0  # rank 5
    card_block[0, 13 + 0] = 1.0
    card_block[1, 3] = 1.0
    card_block[1, 13 + 1] = 1.0
    card_block[2, 10] = 1.0
    card_block[2, 13 + 2] = 1.0

    obs[OFF_HAND_CARDS : OFF_HAND_CARDS + HAND_SLOTS * HAND_CARD_FEATURES] = card_block.reshape(-1)
    obs[:ACTION_DIM] = 1.0
    return obs


def test_greedy_uses_dynamic_offsets_and_prefers_play_for_pair() -> None:
    agent = GreedyAgent(play_threshold=1)
    obs = _build_obs_with_pair()
    action_mask = np.ones(ACTION_DIM, dtype=bool)

    action = agent.act(obs, info=None, action_mask=action_mask)
    assert action == PLAY_INDEX


def test_greedy_discards_when_no_play_signal() -> None:
    agent = GreedyAgent(play_threshold=8)
    obs = _build_obs_with_pair()
    action_mask = np.ones(ACTION_DIM, dtype=bool)
    action_mask[PLAY_INDEX] = False

    action = agent.act(obs, info=None, action_mask=action_mask)
    assert action == DISCARD_INDEX
