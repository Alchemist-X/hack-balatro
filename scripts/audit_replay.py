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

    hard_errors = [issue for issue in issues if issue["severity"] == "error"]
    warnings = [issue for issue in issues if issue["severity"] == "warning"]
    return {
        "ok": not hard_errors and not warnings,
        "hard_invariants_ok": not hard_errors,
        "fidelity_ready": not hard_errors and not warnings,
        "source_oracle_path": source_oracle.get("_path") if source_oracle else None,
        "issues": issues,
        "summary": {
            "transitions": len(transitions),
            "seen_lua_states": sorted(state for state in seen_lua_states if state),
            "seen_transient_lua_states": sorted(state for state in seen_transient_lua_states if state),
            "seen_rng_domains": sorted(domain for domain in seen_rng_domains if domain),
            "unsupported_notes": sorted(unsupported_notes),
            "hard_error_count": len(hard_errors),
            "warning_count": len(warnings),
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
