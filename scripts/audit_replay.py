#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


EXPECTED_STABLE_LUA_STATES = {
    "BLIND_SELECT",
    "SELECTING_HAND",
    "ROUND_EVAL",
    "SHOP",
    "GAME_OVER",
}

EXPECTED_TRANSIENT_LUA_STATES = {
    "NEW_ROUND",
    "DRAW_TO_HAND",
    "HAND_PLAYED",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit replay transitions against extracted Balatro flow invariants")
    parser.add_argument("--replay", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--source-oracle", type=Path, default=None)
    return parser.parse_args()


def add_issue(issues: list[dict[str, Any]], severity: str, step: int | None, message: str) -> None:
    issues.append({"severity": severity, "step": step, "message": message})


def rng_domains(trace: dict[str, Any]) -> list[str]:
    return [str(entry.get("domain", "")) for entry in trace.get("rng_calls", [])]


def has_rng_prefix(trace: dict[str, Any], prefix: str) -> bool:
    return any(domain.startswith(prefix) for domain in rng_domains(trace))


########################################################################
# Numerical checks (Check A–E)
########################################################################


def _check_money_conservation(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Check A: verify money deltas are explainable by the action type."""
    passed = 0
    failed = 0
    details: list[dict[str, Any]] = []

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]

        before_money = before.get("money", 0)
        after_money = after.get("money", 0)
        actual_delta = after_money - before_money

        expected_delta: int | None = None
        explanation = ""

        if action_name == "cashout":
            reward = before.get("reward", 0)
            expected_delta = reward
            explanation = f"cashout: +{reward} reward"

        elif action_name.startswith("buy_shop_item_"):
            item_idx = int(action_name.rsplit("_", 1)[1])
            shop_jokers = before.get("shop_jokers", [])
            if item_idx < len(shop_jokers):
                cost = shop_jokers[item_idx].get("buy_cost") or shop_jokers[item_idx].get("cost", 0)
                expected_delta = -cost
                explanation = f"buy joker: -{cost} cost"

        elif action_name.startswith("buy_consumable_"):
            item_idx = int(action_name.rsplit("_", 1)[1])
            shop_consumables = before.get("shop_consumables", [])
            if item_idx < len(shop_consumables):
                cost = shop_consumables[item_idx].get("buy_cost") or shop_consumables[item_idx].get("cost", 0)
                expected_delta = -cost
                explanation = f"buy consumable: -{cost} cost"

        elif action_name == "buy_voucher":
            # Voucher cost may vary; just flag if money didn't decrease
            if actual_delta >= 0:
                failed += 1
                details.append({
                    "step": index,
                    "action": action_name,
                    "message": f"buy_voucher should decrease money but delta={actual_delta}",
                })
            else:
                passed += 1
            continue

        elif action_name.startswith("sell_joker_") or action_name.startswith("sell_consumable_"):
            # Selling should increase money; exact amount depends on sell_value
            if actual_delta <= 0:
                failed += 1
                details.append({
                    "step": index,
                    "action": action_name,
                    "message": f"sell action should increase money but delta={actual_delta}",
                })
            else:
                passed += 1
            continue

        elif action_name == "reroll_shop":
            reroll_cost = before.get("shop_reroll_cost", 5)
            expected_delta = -reroll_cost
            explanation = f"reroll: -{reroll_cost} cost"

        elif action_name in ("play", "discard"):
            # Money should not change during play/discard (ignoring economy jokers)
            expected_delta = 0
            explanation = "play/discard: no money change expected"

        elif action_name == "next_round":
            expected_delta = 0
            explanation = "next_round: no money change expected"

        elif action_name.startswith("select_blind_"):
            expected_delta = 0
            explanation = "select_blind: no money change expected"

        elif action_name.startswith("select_card_") or action_name.startswith("deselect_card_"):
            expected_delta = 0
            explanation = "card selection: no money change expected"

        else:
            # Unknown action — skip
            continue

        if expected_delta is not None:
            if actual_delta == expected_delta:
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "action": action_name,
                    "expected_delta": expected_delta,
                    "actual_delta": actual_delta,
                    "explanation": explanation,
                    "message": f"money delta mismatch: expected {expected_delta}, got {actual_delta}",
                })

    return {"passed": passed, "failed": failed, "details": details}


def _check_score_monotonicity(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Check B: score only increases on play, unchanged on discard, resets on blind entry."""
    passed = 0
    failed = 0
    details: list[dict[str, Any]] = []

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]

        before_score = before.get("score", 0)
        after_score = after.get("score", 0)

        if before.get("stage") == "Stage_Blind" and action_name == "play":
            if after_score < before_score:
                failed += 1
                details.append({
                    "step": index,
                    "message": f"score decreased on play: {before_score} -> {after_score}",
                })
            else:
                passed += 1

        elif before.get("stage") == "Stage_Blind" and action_name == "discard":
            if after_score != before_score:
                failed += 1
                details.append({
                    "step": index,
                    "message": f"score changed on discard: {before_score} -> {after_score}",
                })
            else:
                passed += 1

        elif action_name == "cashout":
            # After cashout and entering shop, score should be retained or reset
            # (score resets happen on blind entry, not cashout itself)
            passed += 1

        elif action_name.startswith("select_blind_"):
            if after_score != 0:
                failed += 1
                details.append({
                    "step": index,
                    "message": f"score not reset on blind entry: {after_score}",
                })
            else:
                passed += 1

    return {"passed": passed, "failed": failed, "details": details}


def _check_chips_mult_from_trace(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Check C: if trace has scoring events, verify score delta consistency."""
    passed = 0
    failed = 0
    details: list[dict[str, Any]] = []

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]
        trace = transition.get("trace", {}) or {}

        if action_name != "play":
            continue
        if before.get("stage") != "Stage_Blind":
            continue

        score_delta = after.get("score", 0) - before.get("score", 0)

        # Look for scoring info in trace
        joker_res = trace.get("joker_resolution", [])
        # Check if trace has a score_breakdown or final_chips/final_mult
        final_chips = trace.get("final_chips")
        final_mult = trace.get("final_mult")

        if final_chips is not None and final_mult is not None:
            expected_score = int(final_chips) * int(final_mult)
            if expected_score == score_delta:
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "final_chips": final_chips,
                    "final_mult": final_mult,
                    "expected_score_delta": expected_score,
                    "actual_score_delta": score_delta,
                    "message": f"chips*mult={expected_score} != score delta={score_delta}",
                })
        else:
            # No explicit chips/mult in trace yet — skip (not a failure)
            pass

    return {"passed": passed, "failed": failed, "skipped_no_trace": True, "details": details}


def _check_hand_discard_counters(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Check D: play decrements plays by 1, discard decrements discards by 1."""
    passed = 0
    failed = 0
    details: list[dict[str, Any]] = []

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]

        if before.get("stage") != "Stage_Blind":
            continue

        before_plays = before.get("plays", 0)
        after_plays = after.get("plays", 0)
        before_discards = before.get("discards", 0)
        after_discards = after.get("discards", 0)

        if action_name == "play":
            # plays should decrement by 1
            if after_plays == before_plays - 1:
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "expected_plays": before_plays - 1,
                    "actual_plays": after_plays,
                    "message": f"play should decrement plays: {before_plays} -> expected {before_plays - 1}, got {after_plays}",
                })

            # discards should not change on play
            if after_discards != before_discards:
                failed += 1
                details.append({
                    "step": index,
                    "message": f"discards changed on play: {before_discards} -> {after_discards}",
                })

        elif action_name == "discard":
            # discards should decrement by 1
            if after_discards == before_discards - 1:
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "expected_discards": before_discards - 1,
                    "actual_discards": after_discards,
                    "message": f"discard should decrement discards: {before_discards} -> expected {before_discards - 1}, got {after_discards}",
                })

            # plays should not change on discard
            if after_plays != before_plays:
                failed += 1
                details.append({
                    "step": index,
                    "message": f"plays changed on discard: {before_plays} -> {after_plays}",
                })

    return {"passed": passed, "failed": failed, "details": details}


def _check_joker_count_consistency(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Check E: buying a joker increments count, selling decrements, never exceeds slot limit."""
    passed = 0
    failed = 0
    details: list[dict[str, Any]] = []

    # Default Balatro joker slot limit is 5
    DEFAULT_JOKER_SLOT_LIMIT = 5

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]

        before_joker_count = len(before.get("jokers", []) or [])
        after_joker_count = len(after.get("jokers", []) or [])

        if action_name.startswith("buy_shop_item_"):
            # If a joker was bought, count should increase by 1
            if after_joker_count == before_joker_count + 1:
                passed += 1
            elif after_joker_count == before_joker_count:
                # Might have bought a non-joker item (consumable, etc.) — pass
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "action": action_name,
                    "before_count": before_joker_count,
                    "after_count": after_joker_count,
                    "message": f"unexpected joker count change on buy: {before_joker_count} -> {after_joker_count}",
                })

        elif action_name.startswith("sell_joker_"):
            if after_joker_count == before_joker_count - 1:
                passed += 1
            else:
                failed += 1
                details.append({
                    "step": index,
                    "action": action_name,
                    "before_count": before_joker_count,
                    "after_count": after_joker_count,
                    "message": f"unexpected joker count change on sell: {before_joker_count} -> {after_joker_count}",
                })

        # Check slot limit on every transition
        joker_slot_limit = before.get("joker_slot_limit", DEFAULT_JOKER_SLOT_LIMIT)
        if after_joker_count > joker_slot_limit:
            failed += 1
            details.append({
                "step": index,
                "action": action_name,
                "joker_count": after_joker_count,
                "slot_limit": joker_slot_limit,
                "message": f"joker count {after_joker_count} exceeds slot limit {joker_slot_limit}",
            })

    return {"passed": passed, "failed": failed, "details": details}


def run_numerical_checks(transitions: list[dict[str, Any]]) -> dict[str, Any]:
    """Run all numerical checks and return combined results."""
    return {
        "money_conservation": _check_money_conservation(transitions),
        "score_monotonicity": _check_score_monotonicity(transitions),
        "chips_mult_from_trace": _check_chips_mult_from_trace(transitions),
        "hand_discard_tracking": _check_hand_discard_counters(transitions),
        "joker_count_consistency": _check_joker_count_consistency(transitions),
    }


def audit_replay(replay: dict[str, Any], source_oracle: dict[str, Any] | None = None) -> dict[str, Any]:
    transitions = replay.get("transitions", [])
    issues: list[dict[str, Any]] = []
    seen_lua_states = set()
    seen_transient_lua_states = set()
    seen_rng_domains = set()
    unsupported_notes = set()

    stable_expectation = EXPECTED_STABLE_LUA_STATES
    transient_expectation = EXPECTED_TRANSIENT_LUA_STATES
    if source_oracle is not None:
        stable_expectation = set(source_oracle.get("states", {}).get("stable", stable_expectation))
        transient_expectation = set(source_oracle.get("states", {}).get("transient", transient_expectation))

    for index, transition in enumerate(transitions):
        before = transition["snapshot_before"]
        after = transition["snapshot_after"]
        action_name = transition["action"]["name"]
        trace = transition.get("trace", {}) or {}
        transients = set(trace.get("transient_lua_states", []) or [])
        notes = set(trace.get("notes", []) or [])

        seen_lua_states.add(after.get("lua_state"))
        seen_transient_lua_states.update(transients)
        seen_rng_domains.update(rng_domains(trace))
        unsupported_notes.update(notes)

        stage = after.get("stage")
        lua_state = after.get("lua_state")
        if stage == "Stage_PreBlind" and lua_state != "BLIND_SELECT":
            add_issue(issues, "error", index, f"Stage_PreBlind 应映射到 BLIND_SELECT，但拿到 {lua_state}")
        if stage == "Stage_Blind" and lua_state != "SELECTING_HAND":
            add_issue(issues, "error", index, f"Stage_Blind 应映射到 SELECTING_HAND，但拿到 {lua_state}")
        if stage == "Stage_PostBlind" and lua_state != "ROUND_EVAL":
            add_issue(issues, "error", index, f"Stage_PostBlind 应映射到 ROUND_EVAL，但拿到 {lua_state}")
        if stage == "Stage_Shop" and lua_state != "SHOP":
            add_issue(issues, "error", index, f"Stage_Shop 应映射到 SHOP，但拿到 {lua_state}")

        if before.get("stage") == "Stage_PreBlind" and action_name.startswith("select_blind_"):
            if after.get("stage") != "Stage_Blind":
                add_issue(issues, "error", index, f"select_blind 之后应进入 Stage_Blind，但拿到 {after.get('stage')}")
            if "NEW_ROUND" not in transients or "DRAW_TO_HAND" not in transients:
                add_issue(issues, "error", index, "select_blind 缺少 NEW_ROUND/DRAW_TO_HAND 中间态 trace")
            if not has_rng_prefix(trace, "deck.shuffle.enter_blind"):
                add_issue(issues, "error", index, "进入 blind 时缺少 deck.shuffle.enter_blind RNG trace")

        if before.get("stage") == "Stage_Blind" and action_name == "discard":
            if "DRAW_TO_HAND" not in transients:
                add_issue(issues, "error", index, "discard 之后应出现 DRAW_TO_HAND 中间态 trace")

        if before.get("stage") == "Stage_Blind" and action_name == "play":
            if "HAND_PLAYED" not in transients:
                add_issue(issues, "error", index, "play 之后缺少 HAND_PLAYED 中间态 trace")
            if after.get("stage") == "Stage_Blind" and "DRAW_TO_HAND" not in transients:
                add_issue(issues, "error", index, "未清 blind 的 play 之后应出现 DRAW_TO_HAND 中间态 trace")
            if after.get("stage") == "Stage_PostBlind":
                if "NEW_ROUND" not in transients:
                    add_issue(issues, "error", index, "清 blind 的 play 之后应出现 NEW_ROUND 中间态 trace")
                if "DRAW_TO_HAND" in transients:
                    add_issue(issues, "error", index, "清 blind 的 play 不应再补牌；trace 不应包含 DRAW_TO_HAND")

            before_jokers = list(before.get("jokers", []) or [])
            joker_resolution = list(trace.get("joker_resolution", []) or [])
            if before_jokers:
                if len(joker_resolution) != len(before_jokers):
                    add_issue(
                        issues,
                        "error",
                        index,
                        f"Joker 顺序 trace 数量应为 {len(before_jokers)}，但拿到 {len(joker_resolution)}",
                    )
                for expected, actual in zip(before_jokers, joker_resolution):
                    if actual.get("joker_id") != expected.get("joker_id"):
                        add_issue(
                            issues,
                            "error",
                            index,
                            "Joker 解析顺序与持有顺序不一致: "
                            f"expected {expected.get('joker_id')} got {actual.get('joker_id')}",
                        )
                    if actual.get("slot_index") != expected.get("slot_index"):
                        add_issue(
                            issues,
                            "error",
                            index,
                            "Joker slot_index 顺序不一致: "
                            f"expected {expected.get('slot_index')} got {actual.get('slot_index')}",
                        )
                if not trace.get("retrigger_supported", False):
                    add_issue(issues, "warning", index, "当前 engine 尚未实现 retrigger trace，对 Blueprint/Red Seal 等仍不可信")

        if before.get("stage") == "Stage_PostBlind":
            expected_money = before.get("money", 0) + before.get("reward", 0)
            if action_name != "cashout":
                add_issue(issues, "error", index, f"PostBlind 之后的唯一合法动作应该是 cashout，但拿到 {action_name}")
            if after.get("stage") != "Stage_Shop":
                add_issue(issues, "error", index, f"cashout 之后应进入 Stage_Shop，但拿到 {after.get('stage')}")
            if after.get("money") != expected_money:
                add_issue(
                    issues,
                    "error",
                    index,
                    f"cashout 后金钱应为 {expected_money}，但拿到 {after.get('money')}",
                )
            if not has_rng_prefix(trace, "deck.shuffle.cashout"):
                add_issue(issues, "error", index, "cashout 之后缺少 deck.shuffle.cashout RNG trace")
            if not has_rng_prefix(trace, "cashout_shop_refresh"):
                add_issue(issues, "error", index, "cashout 之后缺少 shop refresh RNG trace")

        if before.get("stage") == "Stage_Shop" and action_name == "reroll_shop":
            if not has_rng_prefix(trace, "reroll_shop_refresh"):
                add_issue(issues, "error", index, "reroll_shop 之后缺少 reroll_shop_refresh RNG trace")

        if before.get("stage") == "Stage_Shop" and action_name == "next_round":
            boss_defeated = before.get("blind_states", {}).get("Boss") == "Defeated"
            if after.get("stage") != "Stage_PreBlind":
                add_issue(issues, "error", index, f"next_round 之后应回到 Stage_PreBlind，但拿到 {after.get('stage')}")
            if boss_defeated:
                if after.get("ante") != before.get("ante", 0) + 1:
                    add_issue(
                        issues,
                        "error",
                        index,
                        f"Boss Shop 之后应升 ante，到 {before.get('ante', 0) + 1}，但拿到 {after.get('ante')}",
                    )
                if after.get("blind_name") != "Small Blind":
                    add_issue(
                        issues,
                        "error",
                        index,
                        f"Boss Shop 之后应回到 Small Blind，但拿到 {after.get('blind_name')}",
                    )
                if not has_rng_prefix(trace, "boss_blind.select"):
                    add_issue(issues, "error", index, "Boss 结算后的 next_round 缺少下个 Boss 选择 RNG trace")
            else:
                if after.get("ante") != before.get("ante"):
                    add_issue(
                        issues,
                        "error",
                        index,
                        f"Small/Big Shop 之后不应升 ante，但从 {before.get('ante')} 变成了 {after.get('ante')}",
                    )

        if "shop_consumables_not_implemented" in notes:
            add_issue(issues, "warning", index, "当前 engine 仍未实现 shop consumables")
        for note in sorted(notes):
            if note.startswith("joker_not_implemented:"):
                add_issue(issues, "warning", index, f"当前 engine 缺少 Joker 原生实现: {note.split(':', 1)[1].strip()}")

    final_lua_state = replay.get("final_snapshot", {}).get("lua_state")
    if final_lua_state:
        seen_lua_states.add(final_lua_state)

    missing_stable = sorted(stable_expectation - seen_lua_states)
    if missing_stable:
        add_issue(
            issues,
            "warning",
            None,
            f"本次 replay 未覆盖这些稳定 Lua 状态: {', '.join(missing_stable)}",
        )

    missing_transient = sorted(transient_expectation - seen_transient_lua_states)
    if missing_transient:
        add_issue(
            issues,
            "warning",
            None,
            "本次 replay 未覆盖这些中间态 trace: " + ", ".join(missing_transient),
        )

    # ── Numerical checks (warnings only) ──────────────────────────────
    numerical_checks = run_numerical_checks(transitions)

    numerical_failure_count = sum(
        check.get("failed", 0) for check in numerical_checks.values()
    )
    for check_name, check_result in numerical_checks.items():
        for detail in check_result.get("details", []):
            add_issue(
                issues,
                "warning",
                detail.get("step"),
                f"[numerical:{check_name}] {detail.get('message', '')}",
            )

    hard_errors = [issue for issue in issues if issue["severity"] == "error"]
    warnings = [issue for issue in issues if issue["severity"] == "warning"]
    return {
        "ok": not hard_errors and not warnings,
        "hard_invariants_ok": not hard_errors,
        "fidelity_ready": not hard_errors and not warnings,
        "source_oracle_path": source_oracle.get("_path") if source_oracle else None,
        "issues": issues,
        "numerical_checks": numerical_checks,
        "summary": {
            "transitions": len(transitions),
            "seen_lua_states": sorted(state for state in seen_lua_states if state),
            "seen_transient_lua_states": sorted(state for state in seen_transient_lua_states if state),
            "seen_rng_domains": sorted(domain for domain in seen_rng_domains if domain),
            "unsupported_notes": sorted(unsupported_notes),
            "hard_error_count": len(hard_errors),
            "warning_count": len(warnings),
            "numerical_failure_count": numerical_failure_count,
        },
    }


def main() -> int:
    args = parse_args()
    replay = json.loads(args.replay.read_text(encoding="utf-8"))
    source_oracle = None
    if args.source_oracle is not None:
        source_oracle = json.loads(args.source_oracle.read_text(encoding="utf-8"))
        source_oracle["_path"] = str(args.source_oracle)
    result = audit_replay(replay, source_oracle=source_oracle)
    rendered = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote {args.output}")
    else:
        print(rendered, end="")
    return 0 if result["fidelity_ready"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
