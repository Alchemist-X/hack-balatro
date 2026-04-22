"""Build fixtures/locale/zh_CN.json from the shipped Balatro zh_CN.lua file.

This is a one-off generator — the generated JSON is committed and consumed at
runtime by env/locale.py. Re-run only if zh_CN.lua changes.

Usage:
    python scripts/build_locale_json.py

Writes to:
    fixtures/locale/zh_CN.json

The parser is intentionally tiny and not a real Lua parser. zh_CN.lua has a
very regular shape:

    <CategoryName>={
        <id>={
            name="<chinese>",
            text={...},
        },
        ...
    },

We locate the top-level category headers by indentation and then regex out
`id = { name = "..."` pairs within each category's byte range.
"""
from __future__ import annotations

import json
import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
ZH_LUA = (
    REPO_ROOT
    / "vendor"
    / "balatro"
    / "steam-local"
    / "extracted"
    / "localization"
    / "zh_CN.lua"
)
OUT_PATH = REPO_ROOT / "fixtures" / "locale" / "zh_CN.json"


# Top-level category headers inside `descriptions = {}` use 8-space indent,
# e.g. "        Back={". Category name -> output key in our JSON.
CATEGORY_HEADER_RE = re.compile(r"^ {8}([A-Z][A-Za-z_]+)=\{\s*$", re.MULTILINE)

# Inside a category: `            <id>={` on 12 spaces, then eventually
# `                name="<chinese>",` on 16 spaces. Match id first, then
# a `name="..."` that follows before the next id opens.
ID_OPEN_RE = re.compile(r"^ {12}([a-zA-Z_][a-zA-Z0-9_]*)=\{\s*$", re.MULTILINE)
NAME_RE = re.compile(r'name="((?:[^"\\]|\\.)*)"')


# Which Lua categories we pull and what key we emit them under in the JSON.
CATEGORY_MAP: dict[str, str] = {
    "Joker": "jokers",
    "Tag": "tags",
    "Blind": "blinds",
    "Planet": "planets",
    "Tarot": "tarots",
    "Spectral": "spectrals",
    "Voucher": "vouchers",
    "Back": "decks_full",  # we merge later
}


# Hand types are not identified by stable keys in zh_CN.lua (they appear inside
# misc tables and text blocks with formatting). Hand-written map below.
HAND_TYPES_ZH: dict[str, str] = {
    "High Card": "高牌",
    "Pair": "对子",
    "Two Pair": "两对",
    "Three of a Kind": "三条",
    "Straight": "顺子",
    "Flush": "同花",
    "Full House": "葫芦",
    "Four of a Kind": "四条",
    "Straight Flush": "同花顺",
    "Five of a Kind": "五条",
    "Flush House": "同花葫芦",
    "Flush Five": "同花五条",
}

# Vanilla decks (15 official decks). zh_CN.lua has these under Back but naming
# differs a bit (e.g. "废弃" vs "遗弃"). We use the task spec's canonical names.
DECKS_ZH: dict[str, str] = {
    "b_red": "红色牌组",
    "b_blue": "蓝色牌组",
    "b_yellow": "黄色牌组",
    "b_green": "绿色牌组",
    "b_black": "黑色牌组",
    "b_magic": "魔术牌组",
    "b_nebula": "星云牌组",
    "b_ghost": "幽灵牌组",
    "b_abandoned": "遗弃牌组",
    "b_checkered": "棋盘牌组",
    "b_zodiac": "星座牌组",
    "b_painted": "彩绘牌组",
    "b_anaglyph": "立体牌组",
    "b_plasma": "等离子牌组",
    "b_erratic": "不规则牌组",
}

RANKS_ZH: dict[str, str] = {
    "2": "2",
    "3": "3",
    "4": "4",
    "5": "5",
    "6": "6",
    "7": "7",
    "8": "8",
    "9": "9",
    "10": "10",
    "J": "J",
    "Q": "Q",
    "K": "K",
    "A": "A",
}

SUITS_ZH: dict[str, str] = {
    "S": "\u2660",  # spade
    "H": "\u2665",  # heart
    "D": "\u2666",  # diamond
    "C": "\u2663",  # club
}

# Hand-written section-header labels used by the state serializer and the
# interactive REPL. Not sourced from zh_CN.lua (those labels live in the
# engine/UI code, not the localization file).
LABELS_ZH: dict[str, str] = {
    "stage": "阶段",
    "ante": "大关",
    "round": "小关",
    "score": "分数",
    "plays": "出牌次数",
    "discards": "弃牌次数",
    "money": "资金",
    "hand": "手牌",
    "jokers": "小丑",
    "consumables": "消耗品",
    "shop_jokers": "商店小丑",
    "shop_consumables": "商店消耗品",
    "reroll_cost": "重掷费用",
    "boss_effect": "Boss效果",
    "legal_actions": "合法动作",
    "deck": "牌组",
    "hand_levels": "牌型等级",
    "selected": "已选",
    "resources": "资源",
    "blind": "盲注",
    "tags": "标签",
    "small_tag": "小盲标签",
    "big_tag": "大盲标签",
}


def _slice_categories(lua_text: str) -> dict[str, str]:
    """Return category-name -> raw lua slice (from its header to next header)."""
    headers = list(CATEGORY_HEADER_RE.finditer(lua_text))
    out: dict[str, str] = {}
    for idx, m in enumerate(headers):
        name = m.group(1)
        start = m.end()
        end = headers[idx + 1].start() if idx + 1 < len(headers) else len(lua_text)
        out[name] = lua_text[start:end]
    return out


def _extract_names(category_body: str) -> dict[str, str]:
    """Within one category's body, pull `id={...name="xxx"...}` pairs."""
    result: dict[str, str] = {}
    id_matches = list(ID_OPEN_RE.finditer(category_body))
    for idx, m in enumerate(id_matches):
        id_ = m.group(1)
        start = m.end()
        end = (
            id_matches[idx + 1].start() if idx + 1 < len(id_matches) else len(category_body)
        )
        slice_ = category_body[start:end]
        name_m = NAME_RE.search(slice_)
        if name_m:
            result[id_] = name_m.group(1)
    return result


def build() -> dict:
    if not ZH_LUA.exists():
        raise FileNotFoundError(f"Cannot find {ZH_LUA}")
    text = ZH_LUA.read_text(encoding="utf-8")
    slices = _slice_categories(text)

    out: dict[str, dict] = {}
    for lua_cat, json_key in CATEGORY_MAP.items():
        body = slices.get(lua_cat, "")
        names = _extract_names(body)
        out[json_key] = names

    # Consumables = Tarot + Planet + Spectral (ids are disjoint: c_* prefix).
    consumables: dict[str, str] = {}
    for src in ("tarots", "planets", "spectrals"):
        consumables.update(out.get(src, {}))
    out["consumables"] = consumables

    # Decks: prefer the hand-written canonical set. Also include any Back ids
    # the parser picked up that we didn't hard-code (challenges, etc).
    decks = dict(DECKS_ZH)
    for k, v in out.pop("decks_full", {}).items():
        decks.setdefault(k, v)
    out["decks"] = decks

    # Append hand-written sections.
    out["hand_types"] = HAND_TYPES_ZH
    out["ranks"] = RANKS_ZH
    out["suits"] = SUITS_ZH
    out["labels"] = LABELS_ZH

    # Sort each section by key for stable diffs.
    out = {k: dict(sorted(v.items())) for k, v in out.items()}
    return out


def main() -> int:
    data = build()
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    counts = {k: len(v) if isinstance(v, dict) else "?" for k, v in data.items()}
    print(f"wrote {OUT_PATH}")
    for k, c in counts.items():
        print(f"  {k}: {c} entries")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
