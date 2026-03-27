from __future__ import annotations

from collections import Counter, defaultdict
from itertools import combinations
import random
from typing import Any


RANK_INDEX = {
    "Two": 0,
    "Three": 1,
    "Four": 2,
    "Five": 3,
    "Six": 4,
    "Seven": 5,
    "Eight": 6,
    "Nine": 7,
    "Ten": 8,
    "Jack": 9,
    "Queen": 10,
    "King": 11,
    "Ace": 12,
}

RANK_CHIPS = {
    "Two": 2,
    "Three": 3,
    "Four": 4,
    "Five": 5,
    "Six": 6,
    "Seven": 7,
    "Eight": 8,
    "Nine": 9,
    "Ten": 10,
    "Jack": 10,
    "Queen": 10,
    "King": 10,
    "Ace": 11,
}

RANK_LABEL = {
    "Two": "2",
    "Three": "3",
    "Four": "4",
    "Five": "5",
    "Six": "6",
    "Seven": "7",
    "Eight": "8",
    "Nine": "9",
    "Ten": "10",
    "Jack": "J",
    "Queen": "Q",
    "King": "K",
    "Ace": "A",
}

HAND_ORDER = {
    "high_card": 0,
    "pair": 1,
    "two_pair": 2,
    "three_of_kind": 3,
    "straight": 4,
    "flush": 5,
    "full_house": 6,
    "four_of_a_kind": 7,
    "straight_flush": 8,
    "five_of_a_kind": 9,
    "flush_house": 10,
    "flush_five": 11,
}

LOG_METADATA = {
    "locales": ["en", "zh"],
    "default_locale": "en",
    "terminology_mode": "canonical_en_terms",
}

TEST_FOCUS = [
    "blind_path_fidelity",
    "shop_visibility",
    "rule_based_coverage",
]


def hand_spec_map(bundle: dict[str, Any]) -> dict[str, dict[str, Any]]:
    return {spec["key"]: spec for spec in bundle.get("hand_specs", [])}


def snapshot_selected(snapshot: dict[str, Any]) -> list[dict[str, Any]]:
    selected_ids = {card["card_id"] for card in snapshot.get("selected", [])}
    selected_slots = set(snapshot.get("selected_slots", []))
    cards = []
    for index, card in enumerate(snapshot.get("available", [])):
        if card["card_id"] in selected_ids or index in selected_slots:
            cards.append(card)
    return cards


def card_label(card: dict[str, Any]) -> str:
    return f"{RANK_LABEL.get(card['rank'], card['rank'])} of {card['suit']}"


def card_chip_value(card: dict[str, Any]) -> int:
    return RANK_CHIPS.get(card["rank"], 0)


def straight_exists(unique_ranks: set[int]) -> bool:
    if len(unique_ranks) < 5:
        return False
    values = sorted(unique_ranks)
    if all(rank in unique_ranks for rank in [0, 1, 2, 3, 12]):
        return True
    return any(
        all(window[index + 1] == window[index] + 1 for index in range(4))
        for window in (values[offset : offset + 5] for offset in range(len(values) - 4))
    )


def classify_hand(cards: list[dict[str, Any]]) -> str:
    rank_counts = [0] * 13
    suit_counts = Counter()
    unique_ranks: set[int] = set()
    for card in cards:
        rank_index = RANK_INDEX[card["rank"]]
        rank_counts[rank_index] += 1
        suit_counts[card["suit"]] += 1
        unique_ranks.add(rank_index)
    max_rank = max(rank_counts, default=0)
    pairs = sum(1 for count in rank_counts if count >= 2)
    has_three = any(count >= 3 for count in rank_counts)
    has_flush = any(count >= 5 for count in suit_counts.values())
    has_straight = straight_exists(unique_ranks)

    if max_rank >= 5 and has_flush:
        return "flush_five"
    if has_flush and has_three and pairs >= 2:
        return "flush_house"
    if max_rank >= 5:
        return "five_of_a_kind"
    if has_straight and has_flush:
        return "straight_flush"
    if max_rank >= 4:
        return "four_of_a_kind"
    if has_three and pairs >= 2:
        return "full_house"
    if has_flush:
        return "flush"
    if has_straight:
        return "straight"
    if has_three:
        return "three_of_kind"
    if pairs >= 2:
        return "two_pair"
    if pairs >= 1:
        return "pair"
    return "high_card"


def evaluate_play(
    snapshot: dict[str, Any],
    hand_specs: dict[str, dict[str, Any]],
    cards_with_slots: list[tuple[int, dict[str, Any]]],
) -> dict[str, Any]:
    cards = [card for _slot, card in cards_with_slots]
    hand_key = classify_hand(cards)
    hand_spec = hand_specs[hand_key]
    chips = hand_spec["base_chips"] + sum(card_chip_value(card) for card in cards)
    mult = hand_spec["base_mult"]
    projected_gain = chips * mult
    projected_total = snapshot["score"] + projected_gain
    clears_blind = projected_total >= snapshot["required_score"]
    remaining = max(0, snapshot["required_score"] - projected_total)
    heuristic = projected_gain
    if clears_blind:
        heuristic += max(snapshot["required_score"], 1) * 10
    heuristic += HAND_ORDER.get(hand_key, 0) * 5
    heuristic -= len(cards) * 0.1
    return {
        "target_slots": [slot for slot, _card in cards_with_slots],
        "predicted_hand_key": hand_key,
        "predicted_hand": hand_spec["name"],
        "projected_gain": projected_gain,
        "projected_total": projected_total,
        "remaining_required": remaining,
        "heuristic": heuristic,
    }


def best_play_plan(snapshot: dict[str, Any], hand_specs: dict[str, dict[str, Any]]) -> dict[str, Any]:
    available = snapshot.get("available", [])
    best: dict[str, Any] | None = None
    max_cards = min(5, len(available))
    for size in range(1, max_cards + 1):
        for combo in combinations(list(enumerate(available)), size):
            candidate = evaluate_play(snapshot, hand_specs, list(combo))
            if best is None:
                best = candidate
                continue
            if (
                candidate["heuristic"],
                candidate["projected_gain"],
                HAND_ORDER.get(candidate["predicted_hand_key"], 0),
                -len(candidate["target_slots"]),
                tuple(-slot for slot in candidate["target_slots"]),
            ) > (
                best["heuristic"],
                best["projected_gain"],
                HAND_ORDER.get(best["predicted_hand_key"], 0),
                -len(best["target_slots"]),
                tuple(-slot for slot in best["target_slots"]),
            ):
                best = candidate
    assert best is not None, "play plan requires at least one available card"
    return best


def straight_shell_slots(available: list[dict[str, Any]]) -> list[int]:
    by_rank: dict[int, list[int]] = defaultdict(list)
    for index, card in enumerate(available):
        by_rank[RANK_INDEX[card["rank"]]].append(index)
    ranks = sorted(by_rank)
    runs: list[list[int]] = []
    current: list[int] = []
    previous: int | None = None
    for rank in ranks:
        if previous is None or rank == previous + 1:
            current.append(rank)
        else:
            if len(current) >= 3:
                runs.append(current[:])
            current = [rank]
        previous = rank
    if len(current) >= 3:
        runs.append(current[:])
    if all(rank in by_rank for rank in [12, 0, 1, 2]):
        runs.append([12, 0, 1, 2])
    if not runs:
        return []
    best_run = max(runs, key=lambda run: (len(run), run[-1] if run else -1))
    return [by_rank[rank][0] for rank in best_run]


def keep_shell_slots(snapshot: dict[str, Any]) -> tuple[list[int], str]:
    available = snapshot.get("available", [])
    by_rank: dict[str, list[int]] = defaultdict(list)
    by_suit: dict[str, list[int]] = defaultdict(list)
    for index, card in enumerate(available):
        by_rank[card["rank"]].append(index)
        by_suit[card["suit"]].append(index)

    max_rank_group = max((len(indices) for indices in by_rank.values()), default=0)
    if max_rank_group >= 2:
        keep = [
            index
            for indices in by_rank.values()
            if len(indices) == max_rank_group
            for index in indices
        ]
        keep.sort()
        return keep, "made_rank_group"

    flush_group = max(by_suit.values(), key=len, default=[])
    if len(flush_group) >= 3:
        return sorted(flush_group), "flush_draw"

    straight_group = straight_shell_slots(available)
    if len(straight_group) >= 3:
        return sorted(straight_group), "straight_draw"

    ordered = sorted(
        enumerate(available),
        key=lambda item: (card_chip_value(item[1]), RANK_INDEX[item[1]["rank"]], -item[0]),
        reverse=True,
    )
    keep = sorted(index for index, _card in ordered[: min(3, len(ordered))])
    return keep, "high_card_shell"


def discard_plan(snapshot: dict[str, Any], best_play: dict[str, Any]) -> dict[str, Any]:
    keep, shell = keep_shell_slots(snapshot)
    discard_slots = [index for index in range(len(snapshot.get("available", []))) if index not in set(keep)]
    if not discard_slots and snapshot.get("available"):
        discard_slots = [min(range(len(snapshot["available"])), key=lambda idx: card_chip_value(snapshot["available"][idx]))]
    return {
        "target_slots": discard_slots,
        "keep_shell": shell,
        "predicted_hand": best_play["predicted_hand"],
        "predicted_hand_key": best_play["predicted_hand_key"],
        "projected_gain": best_play["projected_gain"],
    }


def is_weak_play(snapshot: dict[str, Any], play_plan: dict[str, Any]) -> bool:
    if play_plan["projected_total"] >= snapshot["required_score"]:
        return False
    if play_plan["predicted_hand_key"] == "high_card":
        return True
    if play_plan["predicted_hand_key"] == "pair":
        threshold = max(80, max(0, snapshot["required_score"] - snapshot["score"]) // 3)
        return play_plan["projected_gain"] < threshold
    return False


def shell_label_en(shell: str) -> str:
    return {
        "made_rank_group": "made rank group",
        "flush_draw": "flush draw",
        "straight_draw": "straight draw",
        "high_card_shell": "high-card shell",
    }.get(shell, shell.replace("_", " "))


def shell_label_zh(shell: str) -> str:
    return {
        "made_rank_group": "made rank group",
        "flush_draw": "flush draw",
        "straight_draw": "straight draw",
        "high_card_shell": "high-card shell",
    }.get(shell, shell.replace("_", " "))


def enabled_actions(legal_actions: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {action["name"]: action for action in legal_actions if action.get("enabled")}


def fallback_plan(legal_actions: list[dict[str, Any]], policy_id: str) -> dict[str, Any]:
    if not legal_actions:
        return {
            "policy_id": policy_id,
            "action_index": 0,
            "action_name": "noop",
            "mode": "fallback",
            "final_action": "noop",
            "rationale_tags": ["tempo"],
            "target_slots": [],
            "predicted_hand": None,
        }
    action = legal_actions[0]
    return {
        "policy_id": policy_id,
        "action_index": action["index"],
        "action_name": action["name"],
        "mode": "fallback",
        "final_action": action["name"],
        "rationale_tags": ["tempo"],
        "target_slots": [],
        "predicted_hand": None,
    }


def _selection_plan(
    snapshot: dict[str, Any],
    actions_by_name: dict[str, dict[str, Any]],
    base: dict[str, Any],
    target_slots: list[int],
    final_action: str,
) -> dict[str, Any]:
    current = set(snapshot.get("selected_slots", []))
    target = set(target_slots)
    deselect = sorted(current - target)
    select = sorted(target - current)
    plan = dict(base)
    plan["target_slots"] = sorted(target_slots)
    plan["final_action"] = final_action
    if deselect:
        action_name = f"select_card_{deselect[0]}"
        mode = f"toggle_{final_action}"
    elif select:
        action_name = f"select_card_{select[0]}"
        mode = f"toggle_{final_action}"
    else:
        action_name = final_action
        mode = f"execute_{final_action}"
    action = actions_by_name.get(action_name)
    if action is None:
        return fallback_plan(list(actions_by_name.values()), base["policy_id"])
    plan["action_index"] = action["index"]
    plan["action_name"] = action_name
    plan["mode"] = mode
    return plan


def plan_simple_rule_action(
    snapshot: dict[str, Any],
    legal_actions: list[dict[str, Any]],
    bundle: dict[str, Any],
) -> dict[str, Any]:
    actions = enabled_actions(legal_actions)
    if not actions:
        return fallback_plan(legal_actions, "simple_rule_v1")

    stage = snapshot.get("stage")
    hand_specs = hand_spec_map(bundle)

    if stage == "Stage_PreBlind":
        action = next(
            (
                action
                for name, action in sorted(actions.items())
                if name.startswith("select_blind_")
            ),
            None,
        )
        if action is None:
            return fallback_plan(list(actions.values()), "simple_rule_v1")
        return {
            "policy_id": "simple_rule_v1",
            "action_index": action["index"],
            "action_name": action["name"],
            "mode": "blind_select",
            "final_action": action["name"],
            "rationale_tags": ["blind_selection", "tempo"],
            "target_slots": [],
            "predicted_hand": None,
            "blind_name": snapshot.get("blind_name", "Blind"),
        }

    if stage == "Stage_Blind":
        play_plan = best_play_plan(snapshot, hand_specs)
        should_discard = (
            snapshot.get("discards", 0) > 0
            and snapshot.get("plays", 0) > 1
            and is_weak_play(snapshot, play_plan)
        )
        if snapshot.get("plays", 0) <= 0 and snapshot.get("discards", 0) > 0:
            should_discard = True
        if should_discard:
            discard = discard_plan(snapshot, play_plan)
            return _selection_plan(
                snapshot,
                actions,
                {
                    "policy_id": "simple_rule_v1",
                    "rationale_tags": ["draw_improvement", "tempo"],
                    "predicted_hand": discard["predicted_hand"],
                    "projected_gain": discard["projected_gain"],
                    "keep_shell": discard["keep_shell"],
                },
                discard["target_slots"],
                "discard",
            )
        return _selection_plan(
            snapshot,
            actions,
            {
                "policy_id": "simple_rule_v1",
                "rationale_tags": ["score_push", "hand_quality"],
                "predicted_hand": play_plan["predicted_hand"],
                "projected_gain": play_plan["projected_gain"],
                "remaining_required": play_plan["remaining_required"],
            },
            play_plan["target_slots"],
            "play",
        )

    if stage == "Stage_PostBlind":
        action = actions.get("cashout")
        if action is None:
            return fallback_plan(list(actions.values()), "simple_rule_v1")
        return {
            "policy_id": "simple_rule_v1",
            "action_index": action["index"],
            "action_name": action["name"],
            "mode": "cashout",
            "final_action": action["name"],
            "rationale_tags": ["economy", "tempo"],
            "target_slots": [],
            "predicted_hand": None,
        }

    if stage == "Stage_CashOut":
        action = actions.get("next_round")
        if action is None:
            return fallback_plan(list(actions.values()), "simple_rule_v1")
        return {
            "policy_id": "simple_rule_v1",
            "action_index": action["index"],
            "action_name": action["name"],
            "mode": "next_round",
            "final_action": action["name"],
            "rationale_tags": ["tempo"],
            "target_slots": [],
            "predicted_hand": None,
        }

    if stage == "Stage_Shop":
        candidates = []
        for slot, joker in enumerate(snapshot.get("shop_jokers", [])):
            action_name = f"buy_shop_item_{slot}"
            action = actions.get(action_name)
            if action is None:
                continue
            if joker["cost"] > snapshot.get("money", 0):
                continue
            qualifies = joker["rarity"] >= 2 or (
                joker["rarity"] == 1 and not snapshot.get("jokers")
            )
            score = joker["rarity"] * 100 - joker["cost"] * 5
            if joker["rarity"] == 1 and not snapshot.get("jokers"):
                score += 15
            if qualifies:
                candidates.append((score, slot, joker, action))

        if candidates:
            _score, _slot, joker, action = max(candidates, key=lambda item: item[0])
            return {
                "policy_id": "simple_rule_v1",
                "action_index": action["index"],
                "action_name": action["name"],
                "mode": "buy_joker",
                "final_action": action["name"],
                "rationale_tags": ["shop_value", "economy"],
                "target_slots": [],
                "predicted_hand": None,
                "shop_joker_name": joker["name"],
                "shop_joker_cost": joker["cost"],
                "shop_joker_rarity": joker["rarity"],
            }

        reroll = actions.get("reroll_shop")
        if reroll is not None and snapshot.get("money", 0) >= 6:
            return {
                "policy_id": "simple_rule_v1",
                "action_index": reroll["index"],
                "action_name": reroll["name"],
                "mode": "reroll_shop",
                "final_action": reroll["name"],
                "rationale_tags": ["shop_value", "tempo"],
                "target_slots": [],
                "predicted_hand": None,
            }

        action = actions.get("next_round")
        if action is None:
            return fallback_plan(list(actions.values()), "simple_rule_v1")
        return {
            "policy_id": "simple_rule_v1",
            "action_index": action["index"],
            "action_name": action["name"],
            "mode": "next_round",
            "final_action": action["name"],
            "rationale_tags": ["tempo"],
            "target_slots": [],
            "predicted_hand": None,
        }

    return fallback_plan(list(actions.values()), "simple_rule_v1")


def choose_policy_action(
    snapshot: dict[str, Any],
    legal_actions: list[dict[str, Any]],
    bundle: dict[str, Any],
    policy: str,
    rng: random.Random,
) -> tuple[int, dict[str, Any] | None]:
    enabled = [action for action in legal_actions if action.get("enabled")]
    if not enabled:
        return 0, None
    if policy == "random":
        action = rng.choice(enabled)
        return action["index"], None
    if policy == "simple_rule_v1":
        plan = plan_simple_rule_action(snapshot, legal_actions, bundle)
        return plan["action_index"], plan
    return enabled[0]["index"], None


def summarize_events(events: list[dict[str, Any]]) -> str:
    summaries = [event.get("summary") for event in events if event.get("summary")]
    return "; ".join(summaries) if summaries else "No event summary recorded."


def log_context(
    before: dict[str, Any],
    after: dict[str, Any],
    plan: dict[str, Any],
) -> dict[str, Any]:
    target_slots = [
        slot
        for slot in plan.get("target_slots", [])
        if slot < len(before.get("available", []))
    ]
    selected_cards = [card_label(before["available"][slot]) for slot in target_slots]
    return {
        "stage_before": before.get("stage"),
        "stage_after": after.get("stage"),
        "blind_name": after.get("blind_name") or before.get("blind_name"),
        "score_before": before.get("score"),
        "score_after": after.get("score"),
        "required_score_after": after.get("required_score"),
        "money_before": before.get("money"),
        "money_after": after.get("money"),
        "plays_before": before.get("plays"),
        "plays_after": after.get("plays"),
        "discards_before": before.get("discards"),
        "discards_after": after.get("discards"),
        "selected_cards": selected_cards,
        "predicted_hand": plan.get("predicted_hand"),
    }


def _headline_en(plan: dict[str, Any]) -> str:
    mode = plan.get("mode")
    if mode == "blind_select":
        return f"Select {plan.get('blind_name', 'Blind')}."
    if mode == "toggle_play":
        return f"Build {plan.get('predicted_hand') or 'target'} selection."
    if mode == "execute_play":
        return f"Play {plan.get('predicted_hand') or 'hand'}."
    if mode == "toggle_discard":
        return "Mark discard package."
    if mode == "execute_discard":
        return "Discard for redraw."
    if mode == "buy_joker":
        return f"Buy {plan.get('shop_joker_name', 'Joker')}."
    if mode == "reroll_shop":
        return "Reroll shop."
    if mode == "cashout":
        return "Cash Out."
    if mode == "next_round":
        return "Advance to next round."
    return f"Execute {plan.get('action_name', 'action')}."


def _headline_zh(plan: dict[str, Any]) -> str:
    mode = plan.get("mode")
    if mode == "blind_select":
        return f"选择 {plan.get('blind_name', 'Blind')}。"
    if mode == "toggle_play":
        return f"构建 {plan.get('predicted_hand') or 'target'} 选牌。"
    if mode == "execute_play":
        return f"打出 {plan.get('predicted_hand') or 'hand'}。"
    if mode == "toggle_discard":
        return "标记 discard 组合。"
    if mode == "execute_discard":
        return "执行 discard 换牌。"
    if mode == "buy_joker":
        return f"购买 {plan.get('shop_joker_name', 'Joker')}。"
    if mode == "reroll_shop":
        return "执行 reroll_shop。"
    if mode == "cashout":
        return "执行 Cash Out。"
    if mode == "next_round":
        return "进入下一回合。"
    return f"执行 {plan.get('action_name', 'action')}。"


def _reason_en(before: dict[str, Any], plan: dict[str, Any]) -> str:
    mode = plan.get("mode")
    if mode == "blind_select":
        return (
            f"{plan.get('blind_name', 'This Blind')} is the current selectable blind in the linear blind path, "
            "so taking it preserves round tempo without using a skip."
        )
    if mode == "toggle_play":
        return (
            f"This card belongs to the target {plan.get('predicted_hand') or 'play'} line, "
            "which currently has the best base heuristic in the hand."
        )
    if mode == "execute_play":
        return (
            f"{plan.get('predicted_hand') or 'This line'} is the highest-value base-scoring line "
            "available from the current hand and keeps the run on pace for the required score."
        )
    if mode == "toggle_discard":
        shell = shell_label_en(plan.get("keep_shell", "draw shell"))
        return (
            "This card is outside the strongest keep shell. Marking it for discard improves redraw "
            f"quality around the current {shell}."
        )
    if mode == "execute_discard":
        return (
            f"The current best line is only {plan.get('predicted_hand') or 'a weak hand'}. "
            "With discards still available, redraw equity is worth more than forcing a low-value play."
        )
    if mode == "buy_joker":
        return (
            f"{plan.get('shop_joker_name', 'This Joker')} clears the shop threshold at rarity "
            f"{plan.get('shop_joker_rarity')} for ${plan.get('shop_joker_cost')}."
        )
    if mode == "reroll_shop":
        return (
            "No current shop offer clears the buy threshold. Spending $1 on a reroll is acceptable "
            "at this bankroll level."
        )
    if mode == "cashout":
        return "Cash Out is the only tempo-positive action after clearing the blind."
    if mode == "next_round":
        return "No remaining shop action beats the tempo value of advancing the run."
    return "Fallback action selected from the legal action set."


def _reason_zh(before: dict[str, Any], plan: dict[str, Any]) -> str:
    mode = plan.get("mode")
    if mode == "blind_select":
        return f"{plan.get('blind_name', '该 Blind')} 是当前线性 blind 路径里唯一可进入的目标，因此直接进入它最能保留本回合 tempo。"
    if mode == "toggle_play":
        return (
            f"这张牌属于目标 {plan.get('predicted_hand') or 'play'} 线路，"
            "而这条线在当前手牌里拥有最高的基础 heuristic。"
        )
    if mode == "execute_play":
        return (
            f"{plan.get('predicted_hand') or '这条线路'} 是当前手牌里基础得分最高的可行线路，"
            "并且能让本局继续追赶 required score。"
        )
    if mode == "toggle_discard":
        shell = shell_label_zh(plan.get("keep_shell", "draw shell"))
        return (
            "这张牌不属于最强的 keep shell。先把它标记进 discard，"
            f"可以围绕当前 {shell} 提升 redraw 质量。"
        )
    if mode == "execute_discard":
        return (
            f"当前最好的出牌也只有 {plan.get('predicted_hand') or '弱牌型'}。"
            "在还有 discards 的情况下，redraw equity 高于强行打出低价值手牌。"
        )
    if mode == "buy_joker":
        return (
            f"{plan.get('shop_joker_name', '这个 Joker')} 以 ${plan.get('shop_joker_cost')} "
            f"满足本轮的 shop threshold，rarity 为 {plan.get('shop_joker_rarity')}。"
        )
    if mode == "reroll_shop":
        return "当前 shop 没有任何商品达到买入阈值，所以在当前 bankroll 下花 $1 reroll 是合理的。"
    if mode == "cashout":
        return "清掉 blind 之后，Cash Out 是唯一保持 tempo 的推进动作。"
    if mode == "next_round":
        return "当前没有任何剩余 shop 动作能超过直接推进回合的 tempo 价值。"
    return "从 legal actions 里选择了一个 fallback 动作。"


def _outcome_en(before: dict[str, Any], after: dict[str, Any], plan: dict[str, Any], events: list[dict[str, Any]]) -> str:
    mode = plan.get("mode")
    remaining = max(0, after.get("required_score", 0) - after.get("score", 0))
    if mode == "blind_select":
        return (
            f"Entered {after.get('blind_name')} with {after.get('plays')} Hands, "
            f"{after.get('discards')} Discards, and {after.get('required_score')} required score."
        )
    if mode == "toggle_play":
        current = len(after.get("selected_slots", []))
        target = len(plan.get("target_slots", []))
        return f"Selection is now {current}/{target} cards toward {plan.get('predicted_hand') or 'the target line'}."
    if mode == "execute_play":
        return f"Scored {after.get('score', 0) - before.get('score', 0)} points; {remaining} required score remains."
    if mode == "toggle_discard":
        current = len(after.get("selected_slots", []))
        target = len(plan.get("target_slots", []))
        return f"Marked {current}/{target} discard candidates; discard will resolve once the package is complete."
    if mode == "execute_discard":
        return (
            f"Discard resolved; {after.get('discards')} Discards remain and the hand refilled to "
            f"{len(after.get('available', []))} cards."
        )
    if mode == "buy_joker":
        return f"Bought {plan.get('shop_joker_name', 'Joker')}; bankroll is now ${after.get('money')}."
    if mode == "reroll_shop":
        return f"Shop rerolled; bankroll is now ${after.get('money')}."
    if mode == "cashout":
        return f"Collected ${after.get('money', 0) - before.get('money', 0)} and entered Shop."
    if mode == "next_round":
        return f"Advanced to Ante {after.get('ante')}."
    return summarize_events(events)


def _outcome_zh(before: dict[str, Any], after: dict[str, Any], plan: dict[str, Any], events: list[dict[str, Any]]) -> str:
    mode = plan.get("mode")
    remaining = max(0, after.get("required_score", 0) - after.get("score", 0))
    if mode == "blind_select":
        return (
            f"已进入 {after.get('blind_name')}，当前有 {after.get('plays')} 次 Hands、"
            f"{after.get('discards')} 次 Discards，过关需要 {after.get('required_score')} 分。"
        )
    if mode == "toggle_play":
        current = len(after.get("selected_slots", []))
        target = len(plan.get("target_slots", []))
        return f"当前已完成 {current}/{target} 张目标选牌，继续朝 {plan.get('predicted_hand') or '目标线路'} 构建。"
    if mode == "execute_play":
        return f"本次获得 {after.get('score', 0) - before.get('score', 0)} 分，距离过关还差 {remaining} 分。"
    if mode == "toggle_discard":
        current = len(after.get("selected_slots", []))
        target = len(plan.get("target_slots", []))
        return f"当前已标记 {current}/{target} 张 discard 目标；完成整包后就会执行 discard。"
    if mode == "execute_discard":
        return (
            f"discard 已结算；剩余 {after.get('discards')} 次 Discards，"
            f"手牌已补回到 {len(after.get('available', []))} 张。"
        )
    if mode == "buy_joker":
        return f"已购买 {plan.get('shop_joker_name', 'Joker')}；当前 bankroll 为 ${after.get('money')}。"
    if mode == "reroll_shop":
        return f"shop 已 reroll；当前 bankroll 为 ${after.get('money')}。"
    if mode == "cashout":
        return f"本次 Cash Out 获得 ${after.get('money', 0) - before.get('money', 0)}，并进入 Shop。"
    if mode == "next_round":
        return f"已推进到 Ante {after.get('ante')}。"
    return summarize_events(events)


def build_decision_log(
    before: dict[str, Any],
    after: dict[str, Any],
    events: list[dict[str, Any]],
    plan: dict[str, Any],
) -> dict[str, Any]:
    return {
        "policy_id": plan["policy_id"],
        "rationale_tags": plan.get("rationale_tags", []),
        "context": log_context(before, after, plan),
        "en": {
            "headline": _headline_en(plan),
            "reason": _reason_en(before, plan),
            "outcome": _outcome_en(before, after, plan, events),
        },
        "zh": {
            "headline": _headline_zh(plan),
            "reason": _reason_zh(before, plan),
            "outcome": _outcome_zh(before, after, plan, events),
        },
    }


def build_behavior_log_record(
    *,
    seed: int,
    step_index: int,
    elapsed_ms: int,
    transition: dict[str, Any],
    decision_log: dict[str, Any] | None,
    policy_id: str,
    started_at: str,
    finished_at: str | None,
    test_focus: list[str] | None = None,
) -> dict[str, Any]:
    body = decision_log or {
        "policy_id": policy_id,
        "rationale_tags": [],
        "context": {},
        "en": {"headline": "", "reason": "", "outcome": ""},
        "zh": {"headline": "", "reason": "", "outcome": ""},
    }
    return {
        "seed": seed,
        "policy_id": policy_id,
        "started_at": started_at,
        "finished_at": finished_at,
        "step_index": step_index,
        "elapsed_ms": elapsed_ms,
        "test_focus": list(test_focus or TEST_FOCUS),
        "action": transition.get("action"),
        "rationale_tags": body.get("rationale_tags", []),
        "context": body.get("context", {}),
        "en": body.get("en", {}),
        "zh": body.get("zh", {}),
    }
