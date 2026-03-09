from __future__ import annotations

from typing import Any, Protocol

import numpy as np


class Agent(Protocol):
    def act(self, obs: np.ndarray, info: dict[str, Any] | None = None, action_mask: np.ndarray | None = None):
        ...
