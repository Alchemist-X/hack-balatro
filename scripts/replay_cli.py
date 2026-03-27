#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


STAGE_LABELS_ZH = {
    "Stage_PreBlind": "选盲阶段",
    "Stage_Blind": "出牌阶段",
    "Stage_PostBlind": "结算阶段",
    "Stage_Shop": "商店阶段",
    "Stage_CashOut": "收款阶段",
    "Stage_End": "结束阶段",
}

LUA_STATE_LABELS_ZH = {
    "BLIND_SELECT": "BLIND_SELECT",
    "SELECTING_HAND": "SELECTING_HAND",
    "ROUND_EVAL": "ROUND_EVAL",
    "SHOP": "SHOP",
    "CASH_OUT": "CASH_OUT",
    "GAME_OVER": "GAME_OVER",
}

SUIT_SYMBOL = {
    "Spades": "S",
    "Hearts": "H",
    "Diamonds": "D",
    "Clubs": "C",
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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render replay.json as a Chinese CLI-style play log")
    parser.add_argument("--replay", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--limit", type=int, default=0, help="Only render the first N transitions; 0 means all")
    return parser.parse_args()


def card_label(card: dict[str, Any]) -> str:
    rank = RANK_LABEL.get(card.get("rank", ""), card.get("rank", "?"))
    suit = SUIT_SYMBOL.get(card.get("suit", ""), "?")
    return f"{rank}{suit}"


def render_blind_states(snapshot: dict[str, Any]) -> str:
    blind_states = snapshot.get("blind_states", {})
    keys = ("Small", "Big", "Boss")
    return " | ".join(f"{key}={blind_states.get(key, '?')}" for key in keys)


def render_events(events: list[dict[str, Any]]) -> str:
    if not events:
        return "  - 无事件"
    return "\n".join(f"  - [{event.get('stage', '?')}] {event.get('summary', '')}" for event in events)


def render_decision_log(transition: dict[str, Any]) -> str:
    decision_log = transition.get("decision_log")
    if not decision_log:
        return "  标题: 无\n  原因: 无内置 decision_log\n  结果: 无"
    body = decision_log.get("zh") or decision_log.get("en") or {}
    return "\n".join(
        [
            f"  标题: {body.get('headline', '无')}",
            f"  原因: {body.get('reason', '无')}",
            f"  结果: {body.get('outcome', '无')}",
        ]
    )


def render_transition(index: int, total: int, transition: dict[str, Any]) -> str:
    before = transition["snapshot_before"]
    after = transition["snapshot_after"]
    divider = "=" * 96
    title = (
        f"步 {index + 1:03d}/{total:03d} | 动作 {transition['action']['name']} | "
        f"阶段 {STAGE_LABELS_ZH.get(after['stage'], after['stage'])} | Lua状态 {LUA_STATE_LABELS_ZH.get(after.get('lua_state', ''), after.get('lua_state', '-'))}"
    )
    head = [
        divider,
        title,
        "-" * 96,
        (
            f"盲注: {after['blind_name']} | Ante: {after['ante']} | Stake: {after['stake']} | "
            f"分数: {after['score']} / {after['required_score']} | 金钱: ${after['money']}"
        ),
        (
            f"手数: {after['plays']} | 弃牌: {after['discards']} | "
            f"牌库: {len(after.get('deck', []))} | 弃牌堆: {len(after.get('discarded', []))}"
        ),
        f"Blind进度: {render_blind_states(after)}",
        f"手牌: {render_cards(after.get('available', []))}",
        f"已选: {render_cards(after.get('selected', []), max_items=5)}",
        f"Jokers: {render_cards(after.get('jokers', []), max_items=5)}",
        f"商店: {render_cards(after.get('shop_jokers', []), max_items=5)}",
        "事件链:",
        render_events(transition.get("events", [])),
        "行为日志:",
        render_decision_log(transition),
        (
            f"前态: {before['stage']} / {before.get('lua_state', '-')} -> "
            f"后态: {after['stage']} / {after.get('lua_state', '-')}"
        ),
    ]
    return "\n".join(head)


def joker_card_label(entry: dict[str, Any]) -> str:
    return entry.get("name") or entry.get("joker_name") or entry.get("joker_id") or "Joker"


def render_cards(cards: list[dict[str, Any]], max_items: int = 8) -> str:
    if not cards:
        return "-"
    labels: list[str] = []
    for item in cards[:max_items]:
        if "rank" in item and "suit" in item:
            labels.append(card_label(item))
        else:
            labels.append(joker_card_label(item))
    if len(cards) > max_items:
        labels.append(f"...(+{len(cards) - max_items})")
    return " ".join(labels)


def main() -> int:
    args = parse_args()
    replay = json.loads(args.replay.read_text(encoding="utf-8"))
    transitions = replay.get("transitions", [])
    if args.limit > 0:
        transitions = transitions[: args.limit]

    lines = [
        "#" * 96,
        "Balatro 中文 CLI 回放",
        (
            f"版本: {replay.get('version', '?')} | 引擎: {replay.get('engine', '?')} | "
            f"Seed: {replay.get('seed', '?')} | Policy: {replay.get('policy', '?')}"
        ),
        f"转移数: {len(transitions)}",
        "#" * 96,
    ]
    for index, transition in enumerate(transitions):
        lines.append(render_transition(index, len(transitions), transition))

    rendered = "\n".join(lines) + "\n"
    if args.output is not None:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(rendered, encoding="utf-8")
        print(f"wrote {args.output}")
    else:
        print(rendered, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
