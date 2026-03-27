from __future__ import annotations

import json
from pathlib import Path

from scripts.behavior_log import build_behavior_log_record, build_decision_log, plan_simple_rule_action


def load_bundle() -> dict:
    bundle_path = Path("fixtures/ruleset/balatro-1.0.1o-full.json")
    return json.loads(bundle_path.read_text())


def make_card(card_id: int, rank: str, suit: str) -> dict:
    return {
        "card_id": card_id,
        "rank": rank,
        "suit": suit,
        "enhancement": None,
        "edition": None,
        "seal": None,
    }


def make_snapshot(
    *,
    stage: str,
    available: list[dict] | None = None,
    selected_slots: list[int] | None = None,
    score: int = 0,
    required_score: int = 300,
    plays: int = 4,
    discards: int = 3,
    money: int = 4,
    jokers: list[dict] | None = None,
    shop_jokers: list[dict] | None = None,
) -> dict:
    available = available or []
    selected_slots = selected_slots or []
    return {
        "phase": stage.replace("Stage_", ""),
        "stage": stage,
        "round": 1,
        "ante": 1,
        "stake": 1,
        "blind_name": "Small Blind",
        "boss_effect": "None",
        "score": score,
        "required_score": required_score,
        "plays": plays,
        "discards": discards,
        "money": money,
        "reward": 3,
        "deck": [],
        "available": available,
        "selected": [available[index] for index in selected_slots if index < len(available)],
        "discarded": [],
        "jokers": jokers or [],
        "shop_jokers": shop_jokers or [],
        "selected_slots": selected_slots,
        "won": False,
        "over": False,
    }


def make_action(index: int, name: str) -> dict:
    return {"index": index, "name": name, "enabled": True}


def blind_actions(card_count: int) -> list[dict]:
    actions = [make_action(index, f"select_card_{index}") for index in range(card_count)]
    actions.append(make_action(8, "play"))
    actions.append(make_action(9, "discard"))
    return actions


def test_simple_rule_selects_small_blind() -> None:
    bundle = load_bundle()
    snapshot = make_snapshot(stage="Stage_PreBlind")
    legal_actions = [
        make_action(10, "select_blind_0"),
        make_action(11, "select_blind_1"),
        make_action(12, "select_blind_2"),
        make_action(85, "skip_blind"),
    ]
    plan = plan_simple_rule_action(snapshot, legal_actions, bundle)
    assert plan["action_name"] == "select_blind_0"


def test_simple_rule_plays_clear_pair_when_selection_is_ready() -> None:
    bundle = load_bundle()
    available = [
        make_card(1, "Ace", "Spades"),
        make_card(2, "Ace", "Hearts"),
    ]
    snapshot = make_snapshot(
        stage="Stage_Blind",
        available=available,
        selected_slots=[0, 1],
        score=260,
        required_score=300,
    )
    plan = plan_simple_rule_action(snapshot, blind_actions(len(available)), bundle)
    assert plan["action_name"] == "play"
    assert plan["final_action"] == "play"
    assert plan["predicted_hand"] == "Pair"


def test_simple_rule_discards_weak_high_card_with_live_flush_draw() -> None:
    bundle = load_bundle()
    available = [
        make_card(1, "Ace", "Hearts"),
        make_card(2, "Nine", "Hearts"),
        make_card(3, "Six", "Hearts"),
        make_card(4, "Two", "Hearts"),
        make_card(5, "King", "Clubs"),
        make_card(6, "Queen", "Diamonds"),
        make_card(7, "Seven", "Spades"),
        make_card(8, "Four", "Clubs"),
    ]
    snapshot = make_snapshot(
        stage="Stage_Blind",
        available=available,
        selected_slots=[4, 5, 6, 7],
        plays=3,
        discards=2,
    )
    plan = plan_simple_rule_action(snapshot, blind_actions(len(available)), bundle)
    assert plan["action_name"] == "discard"
    assert plan["final_action"] == "discard"
    assert plan["predicted_hand"] == "High Card"


def test_simple_rule_buys_affordable_uncommon_joker() -> None:
    bundle = load_bundle()
    snapshot = make_snapshot(
        stage="Stage_Shop",
        money=6,
        shop_jokers=[
            {"joker_id": "j_joker", "name": "Joker", "cost": 4, "rarity": 1},
            {"joker_id": "j_zany", "name": "Zany Joker", "cost": 6, "rarity": 2},
        ],
    )
    legal_actions = [
        make_action(14, "buy_shop_item_0"),
        make_action(15, "buy_shop_item_1"),
        make_action(70, "next_round"),
        make_action(79, "reroll_shop"),
    ]
    plan = plan_simple_rule_action(snapshot, legal_actions, bundle)
    assert plan["action_name"] == "buy_shop_item_1"
    assert plan["mode"] == "buy_joker"


def test_simple_rule_rerolls_when_only_low_value_commons_remain() -> None:
    bundle = load_bundle()
    snapshot = make_snapshot(
        stage="Stage_Shop",
        money=6,
        jokers=[{"joker_id": "j_joker", "name": "Joker", "cost": 2, "rarity": 1}],
        shop_jokers=[
            {"joker_id": "j_greedy", "name": "Greedy Joker", "cost": 4, "rarity": 1},
            {"joker_id": "j_lusty", "name": "Lusty Joker", "cost": 4, "rarity": 1},
        ],
    )
    legal_actions = [
        make_action(14, "buy_shop_item_0"),
        make_action(15, "buy_shop_item_1"),
        make_action(70, "next_round"),
        make_action(79, "reroll_shop"),
    ]
    plan = plan_simple_rule_action(snapshot, legal_actions, bundle)
    assert plan["action_name"] == "reroll_shop"


def test_decision_log_is_bilingual_and_numeric() -> None:
    bundle = load_bundle()
    available = [
        make_card(1, "Ace", "Spades"),
        make_card(2, "Ace", "Hearts"),
    ]
    before = make_snapshot(
        stage="Stage_Blind",
        available=available,
        selected_slots=[0, 1],
        score=260,
        required_score=300,
        plays=4,
        discards=3,
        money=4,
    )
    plan = plan_simple_rule_action(before, blind_actions(len(available)), bundle)
    after = make_snapshot(
        stage="Stage_Blind",
        available=available,
        selected_slots=[],
        score=324,
        required_score=300,
        plays=3,
        discards=3,
        money=4,
    )
    events = [{"summary": "Played Pair"}]
    log = build_decision_log(before, after, events, plan)
    assert log["policy_id"] == "simple_rule_v1"
    assert "Pair" in log["en"]["headline"]
    assert "Pair" in log["zh"]["headline"]
    assert log["context"]["score_after"] == 324
    assert log["context"]["selected_cards"] == ["A of Spades", "A of Hearts"]


def test_viewer_contains_decision_log_locale_controls() -> None:
    html = Path("viewer/index.html").read_text()
    assert "Decision Log" in html
    assert "setLocale('en')" in html
    assert "balatroReplayLocale" in html


def test_behavior_log_record_carries_timestamps_and_focus() -> None:
    record = build_behavior_log_record(
        seed=42,
        step_index=3,
        elapsed_ms=17,
        transition={"action": {"name": "play", "index": 8}},
        decision_log={
            "policy_id": "simple_rule_v1",
            "rationale_tags": ["score_push"],
            "context": {"predicted_hand": "Pair"},
            "en": {"headline": "Play Pair.", "reason": "Reason", "outcome": "Outcome"},
            "zh": {"headline": "打出 Pair。", "reason": "原因", "outcome": "结果"},
        },
        policy_id="simple_rule_v1",
        started_at="2026-03-14T06:00:00+00:00",
        finished_at="2026-03-14T06:00:01+00:00",
        test_focus=["blind_path_fidelity"],
    )
    assert record["seed"] == 42
    assert record["elapsed_ms"] == 17
    assert record["started_at"] == "2026-03-14T06:00:00+00:00"
    assert record["finished_at"] == "2026-03-14T06:00:01+00:00"
    assert record["test_focus"] == ["blind_path_fidelity"]
