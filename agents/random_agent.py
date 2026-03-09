from __future__ import annotations

import numpy as np


class RandomAgent:
    def __init__(self, seed: int = 42) -> None:
        self.rng = np.random.default_rng(seed)

    def act(self, obs: np.ndarray, info=None, action_mask: np.ndarray | None = None) -> int:
        del obs, info
        if action_mask is None:
            raise ValueError("RandomAgent requires action_mask")
        valid = np.flatnonzero(action_mask)
        if valid.size == 0:
            return 0
        return int(self.rng.choice(valid))
