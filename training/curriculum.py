from __future__ import annotations

from dataclasses import dataclass
from typing import Any


@dataclass
class CurriculumStage:
    name: str
    max_ante: int
    min_win_rate: float = 0.0
    min_episodes: int = 0


class CurriculumScheduler:
    def __init__(self, stages: list[dict[str, Any]]) -> None:
        if not stages:
            raise ValueError("Curriculum stages cannot be empty")
        self.stages = [CurriculumStage(**stage) for stage in stages]
        self.current_idx = 0

    @property
    def current(self) -> CurriculumStage:
        return self.stages[self.current_idx]

    def maybe_advance(self, win_rate: float, episodes: int) -> tuple[bool, CurriculumStage]:
        if self.current_idx >= len(self.stages) - 1:
            return False, self.current

        stage = self.current
        if win_rate >= stage.min_win_rate and episodes >= stage.min_episodes:
            self.current_idx += 1
            return True, self.current
        return False, stage
