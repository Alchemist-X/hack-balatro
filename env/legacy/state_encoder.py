from __future__ import annotations

import itertools
import re
from dataclasses import dataclass
from typing import Any

import numpy as np

from env.legacy.action_space import ACTION_DIM

# Layout constants
ACTION_MASK_SIZE = ACTION_DIM
NUM_STAGES = 7
NUM_SCALARS = 18
HAND_SLOTS = 8
HAND_CARD_FEATURES = 30
HAND_CARDS_DIM = HAND_SLOTS * HAND_CARD_FEATURES
HAND_TYPE_DIM = 12
DECK_COMP_DIM = 52
DISCARDED_DIM = 52
JOKER_HELD_DIM = 47
JOKER_SHOP_DIM = 10
BOSS_DIM = 28
CONSUMABLE_DIM = 2
VOUCHER_DIM = 10

OFF_ACTION_MASK = 0
OFF_STAGE = OFF_ACTION_MASK + ACTION_MASK_SIZE
OFF_SCALARS = OFF_STAGE + NUM_STAGES
OFF_HAND_CARDS = OFF_SCALARS + NUM_SCALARS
OFF_SELECTED_HAND = OFF_HAND_CARDS + HAND_CARDS_DIM
OFF_BEST_HAND = OFF_SELECTED_HAND + HAND_TYPE_DIM
OFF_DECK = OFF_BEST_HAND + HAND_TYPE_DIM
OFF_DISCARDED = OFF_DECK + DECK_COMP_DIM
OFF_JOKER_HELD = OFF_DISCARDED + DISCARDED_DIM
OFF_JOKER_SHOP = OFF_JOKER_HELD + JOKER_HELD_DIM
OFF_BOSS = OFF_JOKER_SHOP + JOKER_SHOP_DIM
OFF_CONSUMABLE = OFF_BOSS + BOSS_DIM
OFF_VOUCHER = OFF_CONSUMABLE + CONSUMABLE_DIM
OBS_DIM = OFF_VOUCHER + VOUCHER_DIM

STAGES = [
    "Stage_PreBlind",
    "Stage_Blind",
    "Stage_PostBlind",
    "Stage_Shop",
    "Stage_End",
    "Stage_CashOut",
    "Stage_Other",
]

HAND_TYPES = [
    "high_card",
    "pair",
    "two_pair",
    "three_of_kind",
    "straight",
    "flush",
    "full_house",
    "four_of_kind",
    "straight_flush",
    "five_of_kind",
    "flush_house",
    "flush_five",
]

BOSS_EFFECTS = [
    "none",
    "thegoad",
    "thehead",
    "theclub",
    "thewindow",
    "theplant",
    "thepsychic",
    "theneedle",
    "thewater",
    "thewall",
    "theflint",
    "theeye",
    "themouth",
    "thehook",
    "theox",
    "thetooth",
    "themanacle",
    "thearm",
    "theserpent",
    "thepillar",
    "thewheel",
    "thehouse",
    "themark",
    "thefish",
    "violetvessel",
    "ceruleanbell",
    "amberacorn",
    "other",
]

# Enhancement string values from engine -> one-hot index (0-7)
ENHANCEMENT_MAP = {
    "m_bonus": 0,
    "m_mult": 1,
    "m_wild": 2,
    "m_glass": 3,
    "m_steel": 4,
    "m_stone": 5,
    "m_gold": 6,
    "m_lucky": 7,
}
ENHANCEMENT_DIM = 8

# Edition string values from engine -> one-hot index (0-2)
EDITION_MAP = {
    "e_foil": 0,
    "e_holo": 1,
    "e_polychrome": 2,
}
EDITION_DIM = 3

# Voucher effect_key values -> bit index (0-9)
VOUCHER_KEYS = [
    "grabber",
    "wasteful",
    "crystal_ball",
    "antimatter",
    "nacho_tong",
    "paint_brush",
    "clearance_sale",
    "restock",
    "seed_money",
    "recyclomancy",
]

RANK_TEXT = {
    "2": 0,
    "3": 1,
    "4": 2,
    "5": 3,
    "6": 4,
    "7": 5,
    "8": 6,
    "9": 7,
    "10": 8,
    "t": 8,
    "j": 9,
    "q": 10,
    "k": 11,
    "a": 12,
}

SUIT_TEXT = {
    "spade": 0,
    "s": 0,
    "heart": 1,
    "h": 1,
    "diamond": 2,
    "d": 2,
    "club": 3,
    "c": 3,
}


@dataclass
class CardData:
    rank_index: int
    suit_index: int
    selected: float
    chip_value: float
    enhancement_index: int | None = None  # index into ENHANCEMENT_MAP
    edition_index: int | None = None  # index into EDITION_MAP


@dataclass
class HandSummary:
    hand_type_index: int
    score_proxy: float


def _safe_get(obj: Any, key: str, default: Any = None) -> Any:
    if obj is None:
        return default
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def _stage_name(stage_obj: Any) -> str:
    if stage_obj is None:
        return "Stage_Other"
    if isinstance(stage_obj, str):
        return stage_obj
    name = stage_obj.__class__.__name__
    if name and name != "str":
        return name
    stage_repr = str(stage_obj)
    return stage_repr if stage_repr else "Stage_Other"


def _one_hot(size: int, index: int | None) -> np.ndarray:
    vec = np.zeros(size, dtype=np.float32)
    if index is not None and 0 <= index < size:
        vec[index] = 1.0
    return vec


def _check_straight_fast(rank_counts: list[int]) -> bool:
    values = [i for i, c in enumerate(rank_counts) if c > 0]
    if not values:
        return False
    rank_set = set(values)
    if {12, 0, 1, 2, 3}.issubset(rank_set):
        return True
    for start in range(0, 9):
        if all((start + offset) in rank_set for offset in range(5)):
            return True
    return False


def classify_hand_direct(ranks: list[int], suits: list[int]) -> int:
    n = len(ranks)
    if n == 0:
        return 0

    rank_counts = [0] * 13
    suit_counts = [0] * 4
    for r in ranks:
        if 0 <= r < 13:
            rank_counts[r] += 1
    for s in suits:
        if 0 <= s < 4:
            suit_counts[s] += 1

    max_rank_count = max(rank_counts)
    pair_counts = sorted((c for c in rank_counts if c >= 2), reverse=True)
    num_pairs = sum(1 for c in rank_counts if c >= 2)
    has_three = any(c >= 3 for c in rank_counts)
    has_flush = n >= 5 and max(suit_counts) >= 5
    has_straight = n >= 5 and _check_straight_fast(rank_counts)

    if max_rank_count >= 5 and has_flush:
        return 11
    if has_flush and has_three and any(c >= 2 for c in rank_counts if c < 3):
        return 10
    if max_rank_count >= 5:
        return 9
    if has_straight and has_flush:
        return 8
    if max_rank_count >= 4:
        return 7
    if has_three and num_pairs >= 2:
        return 6
    if has_flush:
        return 5
    if has_straight:
        return 4
    if has_three:
        return 3
    if len(pair_counts) >= 2:
        return 2
    if len(pair_counts) >= 1:
        return 1
    return 0


def summarize_best_hand(cards: list[CardData]) -> HandSummary:
    if not cards:
        return HandSummary(hand_type_index=0, score_proxy=0.0)

    best_type = 0
    if len(cards) <= 5:
        idx = classify_hand_direct([c.rank_index for c in cards], [c.suit_index for c in cards])
        best_type = idx
    else:
        for combo_size in range(1, min(5, len(cards)) + 1):
            for combo in itertools.combinations(cards, combo_size):
                idx = classify_hand_direct(
                    [c.rank_index for c in combo],
                    [c.suit_index for c in combo],
                )
                if idx > best_type:
                    best_type = idx

    score_proxy = best_type / (len(HAND_TYPES) - 1)
    return HandSummary(hand_type_index=best_type, score_proxy=float(score_proxy))


def _parse_card_repr(text: str) -> tuple[int | None, int | None]:
    text = text.lower()
    rank_index: int | None = None
    suit_index: int | None = None

    rank_match = re.search(r"\b(10|[2-9]|[tjqka])\b", text)
    if rank_match:
        rank_index = RANK_TEXT.get(rank_match.group(1), None)

    for token, idx in SUIT_TEXT.items():
        if token in text:
            suit_index = idx
            break

    return rank_index, suit_index


def _extract_card(card_obj: Any, selected_lookup: set[int]) -> CardData:
    rank = _safe_get(card_obj, "rank_index")
    suit = _safe_get(card_obj, "suit_index")
    chip = _safe_get(card_obj, "chip_value", 1)

    if rank is None:
        rank_raw = _safe_get(card_obj, "rank")
        rank_str = str(rank_raw).lower() if rank_raw is not None else str(card_obj)
        rank, _ = _parse_card_repr(rank_str)
    if suit is None:
        suit_raw = _safe_get(card_obj, "suit")
        suit_str = str(suit_raw).lower() if suit_raw is not None else str(card_obj)
        _, suit_parsed = _parse_card_repr(suit_str)
        if suit_parsed is None:
            card_repr = str(card_obj)
            _, suit_parsed = _parse_card_repr(card_repr)
        suit = suit_parsed

    if rank is None:
        rank = 0
    if suit is None:
        suit = 0

    card_id = _safe_get(card_obj, "card_id")
    selected = 1.0 if card_id in selected_lookup else float(_safe_get(card_obj, "selected", 0.0))

    # Enhancement: Option<str> from native engine, or str/None from mock/dict
    enhancement_raw = _safe_get(card_obj, "enhancement")
    enhancement_index = None
    if enhancement_raw is not None:
        enhancement_index = ENHANCEMENT_MAP.get(str(enhancement_raw).lower())

    # Edition: Option<str> from native engine, or str/None from mock/dict
    edition_raw = _safe_get(card_obj, "edition")
    edition_index = None
    if edition_raw is not None:
        edition_index = EDITION_MAP.get(str(edition_raw).lower())

    return CardData(
        rank_index=int(np.clip(rank, 0, 12)),
        suit_index=int(np.clip(suit, 0, 3)),
        selected=float(selected),
        chip_value=float(np.clip(chip, 0.0, 11.0)),
        enhancement_index=enhancement_index,
        edition_index=edition_index,
    )


def _boss_index(boss_effect: Any) -> int:
    if boss_effect is None:
        return 0
    text = str(boss_effect).lower()
    if not text or text == "none":
        return 0
    for i, effect in enumerate(BOSS_EFFECTS):
        if effect in text:
            return i
    return len(BOSS_EFFECTS) - 1


def _joker_index(joker_obj: Any) -> int:
    name = str(_safe_get(joker_obj, "joker_name", _safe_get(joker_obj, "name", joker_obj))).strip()
    if not name:
        return 0
    # Stable hashed bucket, deterministic across runs
    return (sum(ord(ch) for ch in name) % JOKER_HELD_DIM)


def _card_to_slot(card: CardData) -> int:
    return card.rank_index * 4 + card.suit_index


def _read_cards(state: Any, field_name: str) -> list[Any]:
    cards = _safe_get(state, field_name, [])
    if cards is None:
        return []
    if isinstance(cards, tuple):
        return list(cards)
    return list(cards)


def encode_pylatro_state(state: Any, action_mask: np.ndarray) -> np.ndarray:
    obs = np.zeros(OBS_DIM, dtype=np.float32)

    mask = np.asarray(action_mask, dtype=np.float32)
    if mask.shape[0] < ACTION_MASK_SIZE:
        padded = np.zeros(ACTION_MASK_SIZE, dtype=np.float32)
        padded[: mask.shape[0]] = mask
        mask = padded
    elif mask.shape[0] > ACTION_MASK_SIZE:
        mask = mask[:ACTION_MASK_SIZE]
    obs[OFF_ACTION_MASK : OFF_ACTION_MASK + ACTION_MASK_SIZE] = mask

    stage_name = _stage_name(_safe_get(state, "stage", "Stage_Other"))
    stage_idx = STAGES.index(stage_name) if stage_name in STAGES else len(STAGES) - 1
    obs[OFF_STAGE : OFF_STAGE + NUM_STAGES] = _one_hot(NUM_STAGES, stage_idx)

    score = float(_safe_get(state, "score", 0.0) or 0.0)
    required = float(_safe_get(state, "required_score", 1.0) or 1.0)
    plays = float(_safe_get(state, "plays", 0.0) or 0.0)
    discards = float(_safe_get(state, "discards", 0.0) or 0.0)
    money = float(_safe_get(state, "money", 0.0) or 0.0)
    round_idx = float(_safe_get(state, "round", 0.0) or 0.0)
    ante = float(_safe_get(state, "ante", 0.0) or 0.0)

    available_raw = _read_cards(state, "available")
    selected_raw = _read_cards(state, "selected")
    deck_raw = _read_cards(state, "deck")
    discarded_raw = _read_cards(state, "discarded")

    selected_ids = {
        int(_safe_get(c, "card_id"))
        for c in selected_raw
        if _safe_get(c, "card_id") is not None
    }

    available_cards = [_extract_card(card, selected_ids) for card in available_raw[:HAND_SLOTS]]
    selected_cards = [_extract_card(card, selected_ids) for card in selected_raw]
    deck_cards = [_extract_card(card, set()) for card in deck_raw]
    discarded_cards = [_extract_card(card, set()) for card in discarded_raw]

    best_summary = summarize_best_hand(available_cards)
    selected_summary = summarize_best_hand(selected_cards)

    jokers_list = list(_safe_get(state, "jokers", []) or [])
    consumables_list = list(_safe_get(state, "consumables", []) or [])
    consumable_slot_limit = float(_safe_get(state, "consumable_slot_limit", 2) or 2)
    hand_size_val = float(len(available_raw)) / 10.0
    joker_slot_limit = float(_safe_get(state, "joker_slot_limit", 5) or 5)

    # Detect shop discount: check owned_vouchers for clearance_sale
    owned_vouchers_raw = list(_safe_get(state, "owned_vouchers", []) or [])
    has_clearance = any("clearance" in str(v).lower() for v in owned_vouchers_raw)
    shop_discount = 0.75 if has_clearance else 1.0

    scalar = np.array(
        [
            score / 100_000.0,
            required / 100_000.0,
            0.0 if required <= 0 else min(score / required, 2.0),
            plays / 10.0,
            discards / 10.0,
            money / 100.0,
            round_idx / 10.0,
            len(available_raw) / 24.0,
            len(selected_raw) / 10.0,
            len(jokers_list) / 5.0,
            len(deck_raw) / 60.0,
            float(mask.sum()) / 79.0,
            ante / 8.0,
            best_summary.score_proxy,
            # New economy scalars (indices 14-17)
            hand_size_val,
            joker_slot_limit / 10.0,
            len(consumables_list) / 5.0,
            shop_discount,
        ],
        dtype=np.float32,
    )
    obs[OFF_SCALARS : OFF_SCALARS + NUM_SCALARS] = scalar

    card_block = np.zeros((HAND_SLOTS, HAND_CARD_FEATURES), dtype=np.float32)
    for i, card in enumerate(available_cards):
        card_block[i, card.rank_index] = 1.0
        card_block[i, 13 + card.suit_index] = 1.0
        card_block[i, 17] = card.selected
        card_block[i, 18] = card.chip_value / 11.0
        # Enhancement one-hot (indices 19-26, 8-dim)
        if card.enhancement_index is not None and 0 <= card.enhancement_index < ENHANCEMENT_DIM:
            card_block[i, 19 + card.enhancement_index] = 1.0
        # Edition one-hot (indices 27-29, 3-dim)
        if card.edition_index is not None and 0 <= card.edition_index < EDITION_DIM:
            card_block[i, 27 + card.edition_index] = 1.0
    obs[OFF_HAND_CARDS : OFF_HAND_CARDS + HAND_CARDS_DIM] = card_block.reshape(-1)

    obs[OFF_SELECTED_HAND : OFF_SELECTED_HAND + HAND_TYPE_DIM] = _one_hot(
        HAND_TYPE_DIM,
        selected_summary.hand_type_index,
    )
    obs[OFF_BEST_HAND : OFF_BEST_HAND + HAND_TYPE_DIM] = _one_hot(
        HAND_TYPE_DIM,
        best_summary.hand_type_index,
    )

    deck_vec = np.zeros(DECK_COMP_DIM, dtype=np.float32)
    for card in deck_cards:
        deck_vec[_card_to_slot(card)] = 1.0
    obs[OFF_DECK : OFF_DECK + DECK_COMP_DIM] = deck_vec

    discarded_vec = np.zeros(DISCARDED_DIM, dtype=np.float32)
    for card in discarded_cards:
        discarded_vec[_card_to_slot(card)] = 1.0
    obs[OFF_DISCARDED : OFF_DISCARDED + DISCARDED_DIM] = discarded_vec

    joker_vec = np.zeros(JOKER_HELD_DIM, dtype=np.float32)
    for joker in jokers_list:
        joker_vec[_joker_index(joker)] = 1.0
    obs[OFF_JOKER_HELD : OFF_JOKER_HELD + JOKER_HELD_DIM] = joker_vec

    shop_vec = np.zeros(JOKER_SHOP_DIM, dtype=np.float32)
    shop = list(_safe_get(state, "shop_jokers", []) or [])
    for i, joker in enumerate(shop[:2]):
        base = i * 5
        idx = _joker_index(joker)
        shop_vec[base] = idx / max(1, JOKER_HELD_DIM)
        name = str(_safe_get(joker, "joker_name", _safe_get(joker, "name", ""))).lower()
        shop_vec[base + 1] = 1.0 if "chip" in name else 0.0
        shop_vec[base + 2] = 1.0 if "mult" in name else 0.0
        shop_vec[base + 3] = 1.0 if "x" in name and "mult" in name else 0.0
        shop_vec[base + 4] = 1.0 if any(k in name for k in ("gold", "interest", "money", "cash")) else 0.0
    obs[OFF_JOKER_SHOP : OFF_JOKER_SHOP + JOKER_SHOP_DIM] = shop_vec

    boss_idx = _boss_index(_safe_get(state, "boss_effect", None))
    obs[OFF_BOSS : OFF_BOSS + BOSS_DIM] = _one_hot(BOSS_DIM, boss_idx)

    # Consumable inventory (2-dim: count / limit, count / 5.0)
    consumable_count = len(consumables_list)
    obs[OFF_CONSUMABLE] = consumable_count / max(1.0, consumable_slot_limit)
    obs[OFF_CONSUMABLE + 1] = consumable_count / 5.0

    # Voucher ownership (10-dim binary)
    voucher_vec = np.zeros(VOUCHER_DIM, dtype=np.float32)
    owned_set = {str(v).lower() for v in owned_vouchers_raw}
    for bit_idx, key in enumerate(VOUCHER_KEYS):
        if any(key in v for v in owned_set):
            voucher_vec[bit_idx] = 1.0
    obs[OFF_VOUCHER : OFF_VOUCHER + VOUCHER_DIM] = voucher_vec

    return obs


def unpack_obs_to_structured(obs: np.ndarray) -> dict[str, np.ndarray]:
    arr = np.asarray(obs, dtype=np.float32)
    if arr.ndim == 1:
        arr = arr[None, :]
    if arr.shape[-1] != OBS_DIM:
        raise ValueError(f"Expected obs dim {OBS_DIM}, got {arr.shape[-1]}")

    batch = arr.shape[0]
    card_features = arr[:, OFF_HAND_CARDS : OFF_HAND_CARDS + HAND_CARDS_DIM].reshape(
        batch,
        HAND_SLOTS,
        HAND_CARD_FEATURES,
    )
    card_mask = np.sum(np.abs(card_features), axis=2) < 1e-6

    joker_held = arr[:, OFF_JOKER_HELD : OFF_JOKER_HELD + JOKER_HELD_DIM]
    joker_ids = np.zeros((batch, 5), dtype=np.int64)
    for b in range(batch):
        active = np.where(joker_held[b] > 0.5)[0][:5]
        if active.size > 0:
            joker_ids[b, : active.size] = active + 1
    joker_mask = joker_ids == 0

    global_features = np.concatenate(
        [
            arr[:, OFF_STAGE : OFF_STAGE + NUM_STAGES],
            arr[:, OFF_SCALARS : OFF_SCALARS + NUM_SCALARS],
            arr[:, OFF_SELECTED_HAND : OFF_SELECTED_HAND + HAND_TYPE_DIM],
            arr[:, OFF_BEST_HAND : OFF_BEST_HAND + HAND_TYPE_DIM],
            arr[:, OFF_DECK : OFF_DECK + DECK_COMP_DIM],
            arr[:, OFF_DISCARDED : OFF_DISCARDED + DISCARDED_DIM],
            arr[:, OFF_JOKER_SHOP : OFF_JOKER_SHOP + JOKER_SHOP_DIM],
            arr[:, OFF_BOSS : OFF_BOSS + BOSS_DIM],
            arr[:, OFF_CONSUMABLE : OFF_CONSUMABLE + CONSUMABLE_DIM],
            arr[:, OFF_VOUCHER : OFF_VOUCHER + VOUCHER_DIM],
        ],
        axis=1,
    )

    action_mask = arr[:, OFF_ACTION_MASK : OFF_ACTION_MASK + ACTION_MASK_SIZE] < 0.5

    return {
        "card_features": card_features,
        "card_mask": card_mask,
        "joker_ids": joker_ids,
        "joker_mask": joker_mask,
        "global_features": global_features,
        "action_mask": action_mask,
    }
