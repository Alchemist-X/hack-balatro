from __future__ import annotations

import math
import random
import json
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
    action_name,
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


def _maybe_import_balatro_native() -> Any | None:
    try:
        import balatro_native

        return balatro_native
    except Exception:
        return None


@dataclass
class _MockCard:
    card_id: int
    rank_index: int
    suit_index: int
    chip_value: int
    enhancement: str | None = None
    edition: str | None = None


@dataclass
class _MockJoker:
    joker_name: str
    joker_cost: int = 3


@dataclass
class _MockState:
    stage: str = "Stage_PreBlind"
    round: int = 1
    blind_name: str = "Small Blind"
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
    consumables: list[Any] = field(default_factory=list)
    consumable_slot_limit: int = 2
    owned_vouchers: list[str] = field(default_factory=list)


_RANK_NAMES = [
    "Two",
    "Three",
    "Four",
    "Five",
    "Six",
    "Seven",
    "Eight",
    "Nine",
    "Ten",
    "Jack",
    "Queen",
    "King",
    "Ace",
]

_SUIT_NAMES = ["Spades", "Hearts", "Diamonds", "Clubs"]


def _serialize_mock_card(card: _MockCard) -> dict[str, Any]:
    return {
        "card_id": card.card_id,
        "rank": _RANK_NAMES[card.rank_index],
        "suit": _SUIT_NAMES[card.suit_index],
        "enhancement": None,
        "edition": None,
        "seal": None,
    }


def _serialize_mock_joker(joker: _MockJoker) -> dict[str, Any]:
    return {
        "joker_id": joker.joker_name.lower().replace(" ", "_"),
        "name": joker.joker_name,
        "cost": joker.joker_cost,
        "rarity": 1,
    }


class MockGameEngine:
    def __init__(self, seed: int | None = None) -> None:
        self._rng = random.Random(seed)
        self.state = _MockState()
        self.is_over = False
        self.is_win = False
        self._next_card_id = 1
        self._current_blind_index = 0
        self._boss_name = "The Hook"
        self._build_new_run()

    def _build_new_run(self) -> None:
        self.state = _MockState(
            blind_name="Small Blind",
            deck=[],
            available=[],
            selected=[],
            discarded=[],
            jokers=[],
            shop_jokers=[],
        )
        self._current_blind_index = 0
        self._boss_name = self._rng.choice(["The Hook", "The Ox", "The Arm", "The Wall"])
        self._set_preblind_state()

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

    def _build_deck(self) -> list[_MockCard]:
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
        return deck

    def _blind_name(self) -> str:
        if self._current_blind_index == 0:
            return "Small Blind"
        if self._current_blind_index == 1:
            return "Big Blind"
        return self._boss_name

    def _required_score_for_blind(self) -> int:
        base = 300 * max(1, self.state.ante)
        if self._current_blind_index == 0:
            return base
        if self._current_blind_index == 1:
            return int(round(base * 1.5))
        return base * 2

    def _blind_reward(self) -> int:
        return [3, 4, 5][self._current_blind_index]

    def _blind_states(self) -> dict[str, str]:
        labels = ["Small", "Big", "Boss"]
        states: dict[str, str] = {}
        for index, label in enumerate(labels):
            if index < self._current_blind_index:
                states[label] = "Defeated"
            elif index == self._current_blind_index:
                if self.state.stage == "Stage_PreBlind":
                    states[label] = "Select"
                elif self.state.stage == "Stage_Blind":
                    states[label] = "Current"
                else:
                    states[label] = "Defeated"
            else:
                states[label] = "Upcoming"
        return states

    def _set_preblind_state(self) -> None:
        self.state.stage = "Stage_PreBlind"
        self.state.blind_name = self._blind_name()
        self.state.boss_effect = self._boss_name if self._current_blind_index == 2 else "None"
        self.state.score = 0
        self.state.required_score = self._required_score_for_blind()
        self.state.plays = 4
        self.state.discards = 3
        self.state.reward = self._blind_reward()
        self.state.deck = []
        self.state.available = []
        self.state.selected = []
        self.state.discarded = []
        self.state.shop_jokers = []

    def _start_current_blind(self) -> None:
        self.state.stage = "Stage_Blind"
        self.state.blind_name = self._blind_name()
        self.state.boss_effect = self._boss_name if self._current_blind_index == 2 else "None"
        self.state.score = 0
        self.state.required_score = self._required_score_for_blind()
        self.state.plays = 4
        self.state.discards = 3
        self.state.reward = self._blind_reward()
        self.state.deck = self._build_deck()
        self.state.available = []
        self.state.selected = []
        self.state.discarded = []
        self._draw_to_hand(8)

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
        self.state.ante = min(8, self.state.ante + 1)
        self._current_blind_index = 0
        self._boss_name = self._rng.choice(["The Hook", "The Ox", "The Arm", "The Wall"])
        self._set_preblind_state()

        if self.state.ante >= 8 and self.state.round > 8:
            self.state.stage = "Stage_End"
            self.is_over = True
            self.is_win = True

    def gen_action_space(self) -> list[int]:
        mask = [0] * ACTION_DIM
        stage = self.state.stage

        if stage == "Stage_PreBlind":
            mask[SELECT_BLIND_START + self._current_blind_index] = 1
            if self._current_blind_index < 2:
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
            if index == SELECT_BLIND_START + self._current_blind_index:
                self._start_current_blind()
            elif index == SKIP_BLIND_INDEX and self._current_blind_index < 2:
                self._current_blind_index += 1
                self._set_preblind_state()

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
                if self._current_blind_index < 2:
                    self._current_blind_index += 1
                    self._set_preblind_state()
                else:
                    self.state.stage = "Stage_Shop"
                    self._refresh_shop()

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

    def snapshot_dict(self) -> dict[str, Any]:
        selected_ids = {card.card_id for card in self.state.selected}
        selected_slots = [index for index, card in enumerate(self.state.available) if card.card_id in selected_ids]
        return {
            "phase": self.state.stage.replace("Stage_", ""),
            "stage": self.state.stage,
            "round": self.state.round,
            "ante": self.state.ante,
            "stake": 1,
            "blind_name": self.state.blind_name,
            "boss_effect": self.state.boss_effect,
            "score": self.state.score,
            "required_score": self.state.required_score,
            "plays": self.state.plays,
            "discards": self.state.discards,
            "money": self.state.money,
            "reward": self.state.reward,
            "deck": [_serialize_mock_card(card) for card in self.state.deck],
            "available": [_serialize_mock_card(card) for card in self.state.available],
            "selected": [_serialize_mock_card(card) for card in self.state.selected],
            "discarded": [_serialize_mock_card(card) for card in self.state.discarded],
            "jokers": [_serialize_mock_joker(joker) for joker in self.state.jokers],
            "shop_jokers": [_serialize_mock_joker(joker) for joker in self.state.shop_jokers] if self.state.stage == "Stage_Shop" else [],
            "blind_states": self._blind_states(),
            "selected_slots": selected_slots,
            "won": self.is_win,
            "over": self.is_over,
        }


class _EngineAdapter:
    def __init__(
        self,
        seed: int | None,
        force_mock: bool = False,
        ruleset_path: str | None = None,
        stake: int = 1,
    ) -> None:
        self.backend = "mock"
        self.pylatro = None
        self.native = None
        self._engine: Any
        self._last_transition: dict[str, Any] | None = None

        if not force_mock:
            self.native = _maybe_import_balatro_native()
            self.pylatro = _maybe_import_pylatro()
        if self.native is not None and not force_mock:
            kwargs: dict[str, Any] = {"seed": int(seed or 42), "stake": int(stake)}
            if ruleset_path:
                kwargs["ruleset_path"] = ruleset_path
            self._engine = self.native.Engine(**kwargs)
            self.backend = "balatro_native"
        elif self.pylatro is None or force_mock:
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

    def snapshot_dict(self) -> dict[str, Any]:
        if self.backend == "balatro_native" and hasattr(self._engine, "snapshot"):
            return json.loads(self._engine.snapshot().to_json())
        if self.backend == "mock" and hasattr(self._engine, "snapshot_dict"):
            return self._engine.snapshot_dict()

        state = self.state
        return {
            "phase": str(getattr(state, "phase", self._stage_name())),
            "stage": self._stage_name(),
            "round": int(getattr(state, "round", 0) or 0),
            "ante": int(getattr(state, "ante", 0) or 0),
            "stake": int(getattr(state, "stake", 1) or 1),
            "blind_name": str(getattr(state, "blind_name", "Unknown Blind") or "Unknown Blind"),
            "boss_effect": str(getattr(state, "boss_effect", "None") or "None"),
            "score": int(getattr(state, "score", 0) or 0),
            "required_score": int(getattr(state, "required_score", 0) or 0),
            "plays": int(getattr(state, "plays", 0) or 0),
            "discards": int(getattr(state, "discards", 0) or 0),
            "money": int(getattr(state, "money", 0) or 0),
            "reward": int(getattr(state, "reward", 0) or 0),
            "deck": [],
            "available": [],
            "selected": [],
            "discarded": [],
            "jokers": [],
            "shop_jokers": [],
            "blind_states": {},
            "selected_slots": [],
            "won": self.is_win,
            "over": self.is_over,
        }

    def _stage_name(self) -> str:
        stage = getattr(self.state, "stage", "Stage_Other")
        if isinstance(stage, str):
            return stage
        return stage.__class__.__name__

    @property
    def last_transition(self) -> dict[str, Any] | None:
        return self._last_transition

    def handle_action_index(self, action: int) -> None:
        raw = list(self._engine.gen_action_space())
        if action >= len(raw):
            raise ValueError(f"Action {action} exceeds backend action space {len(raw)}")
        self._last_transition = None
        before = self.snapshot_dict()
        if self.backend == "balatro_native" and hasattr(self._engine, "step"):
            self._last_transition = json.loads(self._engine.step(int(action)).to_json())
            return

        self._engine.handle_action_index(int(action))
        after = self.snapshot_dict()
        self._last_transition = {
            "snapshot_before": before,
            "action": {
                "index": int(action),
                "name": action_name(int(action)),
                "enabled": True,
            },
            "events": [],
            "snapshot_after": after,
            "terminal": bool(after.get("over", False)),
        }


class BalatroEnv(gym.Env if hasattr(gym, "Env") else object):  # type: ignore[misc]
    metadata = {"render_modes": ["human"]}

    def __init__(self, config: dict[str, Any] | None = None):
        self.config = config or {}
        self.reward_cfg = self.config.get("reward", {})
        env_cfg = self.config.get("env", {})
        self.max_steps = int(env_cfg.get("max_steps", 2000))
        self.disable_reorder_actions = bool(env_cfg.get("disable_reorder_actions", True))
        self.force_mock = bool(env_cfg.get("force_mock", False))
        self.ruleset_path = env_cfg.get("ruleset_path")
        self.stake = int(env_cfg.get("stake", 1))
        self.include_state_snapshot_in_info = bool(env_cfg.get("include_state_snapshot_in_info", False))
        self.include_transition_in_info = bool(env_cfg.get("include_transition_in_info", False))

        self._seed = int(env_cfg.get("seed", 42))
        self._rng = np.random.default_rng(self._seed)
        self._engine = _EngineAdapter(
            seed=self._seed,
            force_mock=self.force_mock,
            ruleset_path=self.ruleset_path,
            stake=self.stake,
        )
        self._step_count = 0
        self._blinds_passed = 0
        self._prev_score = 0.0
        self._prev_money = 0.0
        self._prev_ante = 0.0
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
            "seed": self._seed,
            "blinds_passed": self._blinds_passed,
            "game_won": bool(self._engine.is_win),
            "is_over": bool(self._engine.is_over),
            "engine_backend": self._engine.backend,
        }
        if self.include_state_snapshot_in_info:
            info["state_snapshot"] = self._engine.snapshot_dict()
        if self.include_transition_in_info and self._engine.last_transition is not None:
            info["transition"] = self._engine.last_transition
        return info

    def _compute_reward(self, prev_stage: str, new_stage: str, terminated: bool) -> float:
        state = self._engine.state
        score = float(getattr(state, "score", 0.0) or 0.0)
        required = float(getattr(state, "required_score", 1.0) or 1.0)
        money = float(getattr(state, "money", 0.0) or 0.0)
        ante = float(getattr(state, "ante", 0.0) or 0.0)
        score_delta = score - self._prev_score
        money_delta = money - self._prev_money
        ante_delta = ante - self._prev_ante

        reward = 0.0

        # Terminal rewards
        if terminated and self._engine.is_win:
            reward += float(self.reward_cfg.get("win_reward", 10.0))
            self._prev_score = score
            self._prev_money = money
            self._prev_ante = ante
            return float(reward)
        if terminated:
            reward -= float(self.reward_cfg.get("death_penalty", 1.0))
            self._prev_score = score
            self._prev_money = money
            self._prev_ante = ante
            return float(reward)

        # Blind clear bonus
        if prev_stage != "Stage_PostBlind" and new_stage == "Stage_PostBlind":
            reward += float(self.reward_cfg.get("blind_pass_reward", 1.0))
            self._blinds_passed += 1
            boss_effect = str(getattr(state, "boss_effect", "") or "").lower()
            if boss_effect and boss_effect != "none":
                reward += float(self.reward_cfg.get("boss_pass_reward", 2.0))

        # Score efficiency: reward scoring toward blind requirement during play
        if prev_stage == "Stage_Blind" and new_stage == "Stage_Blind" and score_delta > 0 and required > 0:
            scale = float(self.reward_cfg.get("score_shaping_scale", 0.001))
            reward += scale * (score_delta / max(1.0, required))

        # Money management: small reward for earning money in shop
        if new_stage == "Stage_Shop" and money_delta > 0:
            reward += float(self.reward_cfg.get("money_scale", 0.01)) * money_delta

        # Ante progression: big reward for advancing ante
        if ante_delta > 0:
            reward += float(self.reward_cfg.get("ante_advance_reward", 2.0))

        self._prev_score = score
        self._prev_money = money
        self._prev_ante = ante
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

        self._engine = _EngineAdapter(
            seed=self._seed,
            force_mock=self.force_mock,
            ruleset_path=self.ruleset_path,
            stake=self.stake,
        )
        self._step_count = 0
        self._blinds_passed = 0
        self._prev_score = float(getattr(self._engine.state, "score", 0.0) or 0.0)
        self._prev_money = float(getattr(self._engine.state, "money", 0.0) or 0.0)
        self._prev_ante = float(getattr(self._engine.state, "ante", 0.0) or 0.0)
        self._prev_stage = self._stage_name()

        obs = self._obs()
        info = self._info()
        return obs, info

    def render(self):
        return None


class ParallelBalatroEnvs:
    """Synchronous vectorized environment wrapper used by PPO rollout."""

    def __init__(self, config: dict[str, Any], num_envs: int, seed: int = 0, auto_reset: bool = True) -> None:
        self.envs = [BalatroEnv(config) for _ in range(num_envs)]
        self.num_envs = num_envs
        self.seed = seed
        self.auto_reset = auto_reset

    def reset(self, seeds: list[int] | None = None) -> tuple[np.ndarray, list[dict[str, Any]]]:
        obs = []
        infos = []
        for i, env in enumerate(self.envs):
            seed = seeds[i] if seeds is not None else self.seed + i
            o, info = env.reset(seed=seed)
            obs.append(o)
            infos.append(info)
        return np.asarray(obs, dtype=np.float32), infos

    def step(
        self,
        actions: np.ndarray,
        active_mask: np.ndarray | None = None,
    ) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, list[dict[str, Any]]]:
        next_obs = []
        rewards = []
        terminated = []
        truncated = []
        infos = []
        if active_mask is None:
            active_mask = np.ones(len(self.envs), dtype=bool)
        for env, action, is_active in zip(self.envs, actions.tolist(), active_mask.tolist()):
            if not is_active:
                o = env._obs()
                info = env._info()
                info["skipped_step"] = True
                next_obs.append(o)
                rewards.append(0.0)
                terminated.append(True)
                truncated.append(False)
                infos.append(info)
                continue
            o, r, t, tr, info = env.step(int(action))
            if self.auto_reset and (t or tr):
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


def make_vec_env(config: dict[str, Any], num_envs: int, seed: int = 0, auto_reset: bool = True) -> ParallelBalatroEnvs:
    return ParallelBalatroEnvs(config=config, num_envs=num_envs, seed=seed, auto_reset=auto_reset)
