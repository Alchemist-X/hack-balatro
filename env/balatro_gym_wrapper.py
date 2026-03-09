from __future__ import annotations

import math
import random
from dataclasses import dataclass, field
from types import SimpleNamespace
from typing import Any, Callable

import numpy as np

from env.action_space import (
    ACTION_DIM,
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
    SELL_JOKER_COUNT,
    SELL_JOKER_START,
    SKIP_BLIND_INDEX,
)
from env.state_encoder import OBS_DIM, encode_pylatro_state

try:
    import gymnasium as gym
    from gymnasium import spaces
except ImportError:  # pragma: no cover
    gym = object

    class _Space:
        def __init__(self, *_: Any, **__: Any) -> None:
            pass

    class spaces:  # type: ignore
        Box = _Space
        Discrete = _Space


def _maybe_import_pylatro() -> Any | None:
    try:
        import pylatro

        return pylatro
    except Exception:
        return None


@dataclass
class _MockCard:
    card_id: int
    rank_index: int
    suit_index: int
    chip_value: int


@dataclass
class _MockJoker:
    joker_name: str
    joker_cost: int = 3


@dataclass
class _MockState:
    stage: str = "Stage_PreBlind"
    round: int = 1
    score: int = 0
    required_score: int = 300
    plays: int = 4
    discards: int = 3
    money: int = 4
    ante: int = 1
    reward: int = 3
    boss_effect: str = "None"
    deck: list[_MockCard] = field(default_factory=list)
    available: list[_MockCard] = field(default_factory=list)
    selected: list[_MockCard] = field(default_factory=list)
    discarded: list[_MockCard] = field(default_factory=list)
    jokers: list[_MockJoker] = field(default_factory=list)
    shop_jokers: list[_MockJoker] = field(default_factory=list)


class MockGameEngine:
    def __init__(self, seed: int | None = None) -> None:
        self._rng = random.Random(seed)
        self.state = _MockState()
        self.is_over = False
        self.is_win = False
        self._next_card_id = 1
        self._build_new_run()

    def _build_new_run(self) -> None:
        deck = []
        for rank in range(13):
            for suit in range(4):
                deck.append(
                    _MockCard(
                        card_id=self._next_card_id,
                        rank_index=rank,
                        suit_index=suit,
                        chip_value=min(11, rank + 2),
                    )
                )
                self._next_card_id += 1
        self._rng.shuffle(deck)

        self.state = _MockState(
            deck=deck,
            available=[],
            selected=[],
            discarded=[],
            jokers=[],
            shop_jokers=[],
        )
        self._draw_to_hand(8)
        self._refresh_shop()

    def _refresh_shop(self) -> None:
        names = [
            "TheJoker",
            "JollyJoker",
            "Bull",
            "Golden Joker",
            "Hologram",
            "Cavendish",
        ]
        self.state.shop_jokers = [
            _MockJoker(joker_name=self._rng.choice(names), joker_cost=self._rng.randint(2, 6)),
            _MockJoker(joker_name=self._rng.choice(names), joker_cost=self._rng.randint(2, 6)),
        ]

    def _draw_to_hand(self, target: int) -> None:
        while len(self.state.available) < target and self.state.deck:
            self.state.available.append(self.state.deck.pop())

    def _selected_cards(self) -> list[_MockCard]:
        selected_ids = {c.card_id for c in self.state.selected}
        return [c for c in self.state.available if c.card_id in selected_ids]

    def _play_selected(self) -> None:
        selected = self._selected_cards()
        if not selected and self.state.available:
            selected = [self.state.available[0]]
        if not selected:
            return

        base = sum(c.chip_value for c in selected)
        joker_bonus = len(self.state.jokers) * 3
        gained = max(10, base * 4 + joker_bonus)
        self.state.score += gained
        self.state.plays = max(0, self.state.plays - 1)

        selected_ids = {c.card_id for c in selected}
        remain = []
        for card in self.state.available:
            if card.card_id in selected_ids:
                self.state.discarded.append(card)
            else:
                remain.append(card)
        self.state.available = remain
        self.state.selected = []
        self._draw_to_hand(8)

        if self.state.score >= self.state.required_score:
            self.state.stage = "Stage_PostBlind"
        elif self.state.plays <= 0 and self.state.discards <= 0:
            self.state.stage = "Stage_End"
            self.is_over = True
            self.is_win = False

    def _discard_selected(self) -> None:
        if self.state.discards <= 0:
            return
        self.state.discards -= 1
        selected = self._selected_cards()
        if not selected and self.state.available:
            selected = [min(self.state.available, key=lambda c: c.chip_value)]
        selected_ids = {c.card_id for c in selected}
        remain = []
        for card in self.state.available:
            if card.card_id in selected_ids:
                self.state.discarded.append(card)
            else:
                remain.append(card)
        self.state.available = remain
        self.state.selected = []
        self._draw_to_hand(8)
        if self.state.plays <= 0 and self.state.discards <= 0:
            self.state.stage = "Stage_End"
            self.is_over = True
            self.is_win = False

    def _advance_round(self) -> None:
        self.state.round += 1
        self.state.ante = min(8, 1 + (self.state.round - 1) // 3)
        self.state.score = 0
        self.state.plays = 4
        self.state.discards = 3
        self.state.required_score = int(self.state.required_score * 1.35)
        self.state.stage = "Stage_PreBlind"
        self.state.selected = []
        self.state.discarded = []
        self._draw_to_hand(8)

        if self.state.ante >= 8 and self.state.round > 10:
            self.state.stage = "Stage_End"
            self.is_over = True
            self.is_win = True

    def gen_action_space(self) -> list[int]:
        mask = [0] * ACTION_DIM
        stage = self.state.stage

        if stage == "Stage_PreBlind":
            for i in range(SELECT_BLIND_START, SELECT_BLIND_START + SELECT_BLIND_COUNT):
                mask[i] = 1
            mask[SKIP_BLIND_INDEX] = 1

        elif stage == "Stage_Blind":
            for i in range(SELECT_CARD_START, SELECT_CARD_START + min(SELECT_CARD_COUNT, len(self.state.available))):
                mask[i] = 1
            if self.state.plays > 0:
                mask[PLAY_INDEX] = 1
            if self.state.discards > 0:
                mask[DISCARD_INDEX] = 1

        elif stage == "Stage_PostBlind":
            mask[CASHOUT_INDEX] = 1

        elif stage == "Stage_Shop":
            mask[NEXT_ROUND_INDEX] = 1
            mask[REROLL_SHOP_INDEX] = 1 if self.state.money > 0 else 0
            buy_budget = self.state.money >= 2 and len(self.state.jokers) < 5
            if buy_budget:
                for i in range(BUY_JOKER_START, BUY_JOKER_START + BUY_JOKER_COUNT):
                    mask[i] = 1
            if self.state.jokers:
                for i in range(SELL_JOKER_START, SELL_JOKER_START + SELL_JOKER_COUNT):
                    mask[i] = 1

        elif stage == "Stage_CashOut":
            mask[NEXT_ROUND_INDEX] = 1

        return mask

    def handle_action_index(self, index: int) -> None:
        if self.is_over:
            return
        legal = self.gen_action_space()
        if index < 0 or index >= ACTION_DIM or legal[index] != 1:
            raise ValueError(f"Illegal action for stage {self.state.stage}: {index}")

        if self.state.stage == "Stage_PreBlind":
            if index in range(SELECT_BLIND_START, SELECT_BLIND_START + SELECT_BLIND_COUNT) or index == SKIP_BLIND_INDEX:
                self.state.stage = "Stage_Blind"
                self.state.selected = []

        elif self.state.stage == "Stage_Blind":
            if SELECT_CARD_START <= index < SELECT_CARD_START + SELECT_CARD_COUNT and index - SELECT_CARD_START < len(self.state.available):
                card = self.state.available[index - SELECT_CARD_START]
                selected_ids = {c.card_id for c in self.state.selected}
                if card.card_id in selected_ids:
                    self.state.selected = [c for c in self.state.selected if c.card_id != card.card_id]
                else:
                    self.state.selected.append(card)
            elif index == PLAY_INDEX:
                self._play_selected()
            elif index == DISCARD_INDEX:
                self._discard_selected()

        elif self.state.stage == "Stage_PostBlind":
            if index == CASHOUT_INDEX:
                self.state.money += self.state.reward
                self.state.stage = "Stage_Shop"

        elif self.state.stage == "Stage_Shop":
            if BUY_JOKER_START <= index < BUY_JOKER_START + BUY_JOKER_COUNT and self.state.shop_jokers:
                joker = self.state.shop_jokers[(index - BUY_JOKER_START) % len(self.state.shop_jokers)]
                if self.state.money >= joker.joker_cost and len(self.state.jokers) < 5:
                    self.state.money -= joker.joker_cost
                    self.state.jokers.append(joker)
            elif index == REROLL_SHOP_INDEX and self.state.money >= 1:
                self.state.money -= 1
                self._refresh_shop()
            elif SELL_JOKER_START <= index < SELL_JOKER_START + SELL_JOKER_COUNT and self.state.jokers:
                slot = index - SELL_JOKER_START
                if slot < len(self.state.jokers):
                    self.state.jokers.pop(slot)
                    self.state.money += 1
            elif index == NEXT_ROUND_INDEX:
                self._advance_round()

        elif self.state.stage == "Stage_CashOut" and index == NEXT_ROUND_INDEX:
            self._advance_round()


class _EngineAdapter:
    def __init__(self, seed: int | None, force_mock: bool = False) -> None:
        self.backend = "mock"
        self.pylatro = None
        self._engine: Any

        if not force_mock:
            self.pylatro = _maybe_import_pylatro()
        if self.pylatro is None or force_mock:
            self._engine = MockGameEngine(seed=seed)
            self.backend = "mock"
        else:
            self._engine = self.pylatro.GameEngine()
            self.backend = "pylatro"

    @property
    def state(self) -> Any:
        return self._engine.state

    @property
    def is_over(self) -> bool:
        return bool(getattr(self._engine, "is_over", False))

    @property
    def is_win(self) -> bool:
        return bool(getattr(self._engine, "is_win", False))

    def gen_action_space(self) -> list[int]:
        raw = list(self._engine.gen_action_space())
        if len(raw) < ACTION_DIM:
            raw = raw + [0] * (ACTION_DIM - len(raw))
        elif len(raw) > ACTION_DIM:
            raw = raw[:ACTION_DIM]
        return [1 if x else 0 for x in raw]

    def handle_action_index(self, action: int) -> None:
        raw = list(self._engine.gen_action_space())
        if action >= len(raw):
            raise ValueError(f"Action {action} exceeds backend action space {len(raw)}")
        self._engine.handle_action_index(int(action))


class BalatroEnv(gym.Env if hasattr(gym, "Env") else object):  # type: ignore[misc]
    metadata = {"render_modes": ["human"]}

    def __init__(self, config: dict[str, Any] | None = None):
        self.config = config or {}
        self.reward_cfg = self.config.get("reward", {})
        env_cfg = self.config.get("env", {})
        self.max_steps = int(env_cfg.get("max_steps", 2000))
        self.disable_reorder_actions = bool(env_cfg.get("disable_reorder_actions", True))
        self.force_mock = bool(env_cfg.get("force_mock", False))

        self._seed = int(env_cfg.get("seed", 42))
        self._rng = np.random.default_rng(self._seed)
        self._engine = _EngineAdapter(seed=self._seed, force_mock=self.force_mock)
        self._step_count = 0
        self._blinds_passed = 0
        self._prev_score = 0.0
        self._prev_stage = "Stage_Other"

        self.action_space = spaces.Discrete(ACTION_DIM)
        self.observation_space = spaces.Box(low=-np.inf, high=np.inf, shape=(OBS_DIM,), dtype=np.float32)

    def _stage_name(self) -> str:
        stage = getattr(self._engine.state, "stage", "Stage_Other")
        if isinstance(stage, str):
            return stage
        return stage.__class__.__name__

    def get_action_mask(self) -> np.ndarray:
        mask = np.asarray(self._engine.gen_action_space(), dtype=bool)
        if self.disable_reorder_actions:
            mask[24:70] = False
        if not mask.any():
            mask[0] = True
        return mask

    def _obs(self) -> np.ndarray:
        return encode_pylatro_state(self._engine.state, self.get_action_mask())

    def _info(self) -> dict[str, Any]:
        state = self._engine.state
        stage_name = self._stage_name()
        info = {
            "stage": stage_name,
            "score": float(getattr(state, "score", 0.0) or 0.0),
            "required_score": float(getattr(state, "required_score", 0.0) or 0.0),
            "plays": int(getattr(state, "plays", 0) or 0),
            "discards": int(getattr(state, "discards", 0) or 0),
            "money": int(getattr(state, "money", 0) or 0),
            "round": int(getattr(state, "round", 0) or 0),
            "num_available": len(getattr(state, "available", []) or []),
            "num_selected": len(getattr(state, "selected", []) or []),
            "num_jokers": len(getattr(state, "jokers", []) or []),
            "num_deck": len(getattr(state, "deck", []) or []),
            "step_count": self._step_count,
            "blinds_passed": self._blinds_passed,
            "game_won": bool(self._engine.is_win),
            "is_over": bool(self._engine.is_over),
            "engine_backend": self._engine.backend,
        }
        return info

    def _compute_reward(self, prev_stage: str, new_stage: str, terminated: bool) -> float:
        state = self._engine.state
        score = float(getattr(state, "score", 0.0) or 0.0)
        required = float(getattr(state, "required_score", 1.0) or 1.0)
        score_delta = score - self._prev_score

        reward = 0.0
        if bool(self.reward_cfg.get("use_score_shaping", True)) and score_delta > 0 and required > 0:
            scale = float(self.reward_cfg.get("score_shaping_scale", 0.1))
            reward += scale * math.log1p(score_delta / required)

        if prev_stage != "Stage_PostBlind" and new_stage == "Stage_PostBlind":
            reward += float(self.reward_cfg.get("blind_pass_reward", 0.5))
            self._blinds_passed += 1

        if "boss" in str(getattr(state, "boss_effect", "")).lower() and new_stage == "Stage_PostBlind":
            reward += float(self.reward_cfg.get("boss_pass_reward", 0.0))

        if terminated and self._engine.is_win:
            reward += float(self.reward_cfg.get("win_reward", 10.0))
        elif terminated:
            reward -= float(self.reward_cfg.get("death_penalty", 0.0))

        self._prev_score = score
        return float(reward)

    def step(self, action: int):  # type: ignore[override]
        self._step_count += 1
        action = int(action)

        legal_mask = self.get_action_mask()
        legal_actions = np.where(legal_mask)[0].tolist()
        if not legal_actions:
            legal_actions = [0]

        requested_action = action
        fallback_used = False
        if action < 0 or action >= ACTION_DIM or not legal_mask[action]:
            action = legal_actions[0]
            fallback_used = True

        candidates = [action] + [a for a in legal_actions if a != action][:3]
        error: str | None = None
        executed_action = action
        for candidate in candidates:
            try:
                self._engine.handle_action_index(candidate)
                executed_action = candidate
                error = None
                break
            except Exception as exc:  # pragma: no cover - depends on engine behavior
                error = str(exc)

        terminated = bool(self._engine.is_over)
        truncated = self._step_count >= self.max_steps
        if error is not None:
            terminated = True
            truncated = False

        prev_stage = self._prev_stage
        new_stage = self._stage_name()
        self._prev_stage = new_stage

        reward = -1.0 if error is not None else self._compute_reward(prev_stage, new_stage, terminated)
        obs = self._obs()
        info = self._info()
        info["requested_action"] = requested_action
        info["executed_action"] = executed_action
        info["fallback_used"] = fallback_used
        if error is not None:
            info["engine_error"] = error
        return obs, reward, terminated, truncated, info

    def reset(self, seed: int | None = None, options: dict[str, Any] | None = None):  # type: ignore[override]
        del options
        if seed is not None:
            self._seed = int(seed)
            self._rng = np.random.default_rng(self._seed)

        self._engine = _EngineAdapter(seed=self._seed, force_mock=self.force_mock)
        self._step_count = 0
        self._blinds_passed = 0
        self._prev_score = float(getattr(self._engine.state, "score", 0.0) or 0.0)
        self._prev_stage = self._stage_name()

        obs = self._obs()
        info = self._info()
        return obs, info

    def render(self):
        return None


class ParallelBalatroEnvs:
    """Synchronous vectorized environment wrapper used by PPO rollout."""

    def __init__(self, config: dict[str, Any], num_envs: int, seed: int = 0) -> None:
        self.envs = [BalatroEnv(config) for _ in range(num_envs)]
        self.num_envs = num_envs
        self.seed = seed

    def reset(self) -> tuple[np.ndarray, list[dict[str, Any]]]:
        obs = []
        infos = []
        for i, env in enumerate(self.envs):
            o, info = env.reset(seed=self.seed + i)
            obs.append(o)
            infos.append(info)
        return np.asarray(obs, dtype=np.float32), infos

    def step(self, actions: np.ndarray) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, list[dict[str, Any]]]:
        next_obs = []
        rewards = []
        terminated = []
        truncated = []
        infos = []
        for env, action in zip(self.envs, actions.tolist()):
            o, r, t, tr, info = env.step(int(action))
            if t or tr:
                o, reset_info = env.reset()
                info["auto_reset"] = True
                info["reset_info"] = reset_info
            next_obs.append(o)
            rewards.append(float(r))
            terminated.append(bool(t))
            truncated.append(bool(tr))
            infos.append(info)
        return (
            np.asarray(next_obs, dtype=np.float32),
            np.asarray(rewards, dtype=np.float32),
            np.asarray(terminated, dtype=bool),
            np.asarray(truncated, dtype=bool),
            infos,
        )

    def get_action_masks(self) -> np.ndarray:
        return np.asarray([env.get_action_mask() for env in self.envs], dtype=bool)


def make_vec_env(config: dict[str, Any], num_envs: int, seed: int = 0) -> ParallelBalatroEnvs:
    return ParallelBalatroEnvs(config=config, num_envs=num_envs, seed=seed)
