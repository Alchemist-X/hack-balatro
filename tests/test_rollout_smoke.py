from __future__ import annotations

import numpy as np
import torch

from training.rollout import RolloutBuffer


def test_rollout_buffer_shapes_and_batches() -> None:
    buf = RolloutBuffer(num_envs=2, steps_per_env=4, obs_dim=454, action_dim=86)

    for _ in range(4):
        buf.add(
            obs=np.zeros((2, 454), dtype=np.float32),
            actions=np.zeros(2, dtype=np.int64),
            rewards=np.ones(2, dtype=np.float32),
            dones=np.zeros(2, dtype=bool),
            log_probs=np.zeros(2, dtype=np.float32),
            values=np.zeros(2, dtype=np.float32),
            action_masks=np.ones((2, 86), dtype=bool),
        )

    buf.compute_advantages(last_values=np.zeros(2, dtype=np.float32), last_dones=np.zeros(2, dtype=np.float32))
    batches = list(buf.get_batches(mini_batch_size=4, device=torch.device("cpu")))
    assert len(batches) >= 1
    b0 = batches[0]
    assert b0.obs.shape[1] == 454
    assert b0.action_masks.shape[1] == 86
