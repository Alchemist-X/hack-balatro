"""Serialize Balatro engine state into structured text for LLM consumption.

Converts a snapshot (from balatro_native or dict) into a compact, readable
text block that a language model can reason about.

Usage:
    from env.state_serializer import serialize_state
    text = serialize_state(snapshot, legal_actions)            # English (default)
    text = serialize_state(snapshot, legal_actions, lang="zh") # Chinese (opt-in)

The English output is byte-identical to the pre-locale implementation so that
existing training data and evaluation scripts stay stable.
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from env.locale import label as _label, name as _tr

RANKS = ["2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A"]
SUITS = {"Spades": "S", "Hearts": "H", "Diamonds": "D", "Clubs": "C"}
SUIT_FULL = {"S": "Spades", "H": "Hearts", "D": "Diamonds", "C": "Clubs"}

HAND_TYPES = [
    "High Card", "Pair", "Two Pair", "Three of a Kind", "Straight",
    "Flush", "Full House", "Four of a Kind", "Straight Flush",
    "Five of a Kind", "Flush House", "Flush Five",
]
HAND_BASE_CHIPS = [5, 10, 20, 30, 30, 35, 40, 60, 100, 120, 140, 160]
HAND_BASE_MULT = [1, 2, 2, 3, 4, 4, 4, 7, 8, 12, 14, 16]


def _card_str(card: Any, lang: str = "en") -> str:
    """Convert a card object/dict to short string like 'JS' or '10H [Bonus] {Foil}'."""
    if isinstance(card, dict):
        rank = card.get("rank", "?")
        suit = card.get("suit", "?")
        enhancement = card.get("enhancement")
        edition = card.get("edition")
        seal = card.get("seal")
    else:
        rank_idx = getattr(card, "rank_index", None)
        suit_idx = getattr(card, "suit_index", None)
        rank = RANKS[rank_idx] if rank_idx is not None and 0 <= rank_idx < 13 else "?"
        suit_names = ["Spades", "Hearts", "Diamonds", "Clubs"]
        suit = suit_names[suit_idx] if suit_idx is not None and 0 <= suit_idx < 4 else "?"
        enhancement = getattr(card, "enhancement", None)
        edition = getattr(card, "edition", None)
        seal = getattr(card, "seal", None)

    # Normalize rank display
    if isinstance(rank, str) and rank not in RANKS:
        rank_map = {"Two": "2", "Three": "3", "Four": "4", "Five": "5", "Six": "6",
                    "Seven": "7", "Eight": "8", "Nine": "9", "Ten": "10",
                    "Jack": "J", "Queen": "Q", "King": "K", "Ace": "A"}
        rank = rank_map.get(rank, rank)

    s = SUITS.get(suit, suit[0] if suit else "?")
    if lang != "en":
        s = _tr("suits", s, lang=lang, default=s)
    result = f"{rank}{s}"

    tags = []
    if enhancement:
        enh_short = {"m_bonus": "Bonus", "m_mult": "Mult", "m_wild": "Wild",
                     "m_glass": "Glass", "m_steel": "Steel", "m_stone": "Stone",
                     "m_gold": "Gold", "m_lucky": "Lucky"}
        tags.append(enh_short.get(enhancement, enhancement))
    if edition:
        ed_short = {"e_foil": "Foil", "e_holo": "Holo", "e_polychrome": "Poly", "e_negative": "Neg"}
        tags.append(ed_short.get(edition, edition))
    if seal:
        tags.append(f"{seal}Seal")

    if tags:
        result += f" [{'/'.join(tags)}]"
    return result


def _joker_str(joker: Any, lang: str = "en") -> str:
    """Convert a joker object/dict to display string."""
    if isinstance(joker, dict):
        name = joker.get("name", joker.get("joker_name", "?"))
        joker_id = joker.get("joker_id") or joker.get("id")
        cost = joker.get("cost", joker.get("buy_cost", "?"))
        edition = joker.get("edition")
    else:
        name = getattr(joker, "name", getattr(joker, "joker_name", "?"))
        joker_id = getattr(joker, "joker_id", None) or getattr(joker, "id", None)
        cost = getattr(joker, "cost", getattr(joker, "buy_cost", "?"))
        edition = getattr(joker, "edition", None)

    if lang != "en" and joker_id:
        # Prefer the locale-mapped name if we have one; otherwise keep the
        # engine-provided name (already localized in some cases).
        name = _tr("jokers", joker_id, lang=lang, default=str(name))

    result = str(name)
    if edition:
        ed_short = {"e_foil": "Foil", "e_holo": "Holo", "e_polychrome": "Poly", "e_negative": "Neg"}
        result += f" ({ed_short.get(edition, edition)})"
    return result


def _consumable_str(c: Any, lang: str = "en") -> str:
    if isinstance(c, dict):
        name = c.get("name", c.get("consumable_name", "?"))
        cid = c.get("consumable_id") or c.get("id") or c.get("key")
    else:
        name = getattr(c, "name", getattr(c, "consumable_name", "?"))
        cid = getattr(c, "consumable_id", None) or getattr(c, "id", None)
    if lang != "en" and cid:
        name = _tr("consumables", cid, lang=lang, default=str(name))
    return str(name)


def _get(obj: Any, key: str, default: Any = None) -> Any:
    if isinstance(obj, dict):
        return obj.get(key, default)
    return getattr(obj, key, default)


def _snap_dict(snapshot: Any) -> dict:
    """Normalize snapshot to dict."""
    if isinstance(snapshot, dict):
        return snapshot
    if hasattr(snapshot, "to_json"):
        return json.loads(snapshot.to_json())
    return {}


def _section(key: str, lang: str) -> str:
    """Render a `[SECTION]` bracket label. English preserves the uppercased form."""
    return f"[{_label(key, lang=lang)}]"


def serialize_state(
    snapshot: Any,
    legal_actions: list[str] | None = None,
    lang: str = "en",
) -> str:
    """Serialize game state to structured text for LLM.

    Args:
        snapshot: Engine snapshot (native PySnapshot, or dict from to_json)
        legal_actions: List of legal action name strings
        lang: ``"en"`` (default, byte-identical to pre-locale output) or
              ``"zh"`` for Chinese labels and translated joker/tag/hand names.

    Returns:
        Multi-line text description of the game state.
    """
    d = _snap_dict(snapshot)

    stage = d.get("stage", "?")
    ante = d.get("ante", 1)
    round_num = d.get("round", 1)
    score = d.get("score", 0)
    required = d.get("required_score", 0)
    plays = d.get("plays", 0)
    discards = d.get("discards", 0)
    money = d.get("money", 0)
    blind_name = d.get("blind_name", "?")
    boss_effect = d.get("boss_effect", "")

    # Translate blind name via blind key if present on snapshot (engine doesn't
    # currently expose blind_key in the snapshot shape, so this is a no-op unless
    # added later; the English `blind_name` from the engine is passed through).
    if lang != "en":
        blind_key = d.get("blind_key") or d.get("blind_id")
        if blind_key:
            blind_name = _tr("blinds", blind_key, lang=lang, default=blind_name)

    lines: list[str] = []

    # Header
    stage_short = stage.replace("Stage_", "")
    if lang == "en":
        lines.append(f"[STAGE] {stage_short} | {blind_name}")
        lines.append(f"[ANTE] {ante} | Round {round_num}")
    else:
        lines.append(f"{_section('stage', lang)} {stage_short} | {blind_name}")
        lines.append(f"{_section('ante', lang)} {ante} | {_label('round', lang=lang)} {round_num}")

    # Score and resources
    if stage == "Stage_Blind":
        pct = f" ({100*score//required}%)" if required > 0 else ""
        if lang == "en":
            lines.append(f"[SCORE] {score}/{required}{pct}")
        else:
            lines.append(f"{_section('score', lang)} {score}/{required}{pct}")
    if lang == "en":
        lines.append(f"[RESOURCES] Plays: {plays} | Discards: {discards} | Money: ${money}")
    else:
        lines.append(
            f"{_section('resources', lang)} "
            f"{_label('plays', lang=lang)}: {plays} | "
            f"{_label('discards', lang=lang)}: {discards} | "
            f"{_label('money', lang=lang)}: ${money}"
        )

    # Boss effect
    if boss_effect and boss_effect != "none" and boss_effect != "":
        if lang == "en":
            lines.append(f"[BOSS EFFECT] {boss_effect}")
        else:
            lines.append(f"{_section('boss_effect', lang)} {boss_effect}")

    # Hand cards
    available = d.get("available", [])
    selected_slots = set(d.get("selected_slots", []))
    if available:
        hand_strs = []
        for i, card in enumerate(available):
            cs = _card_str(card, lang=lang)
            if i in selected_slots:
                cs = f"*{cs}*"
            hand_strs.append(cs)
        if lang == "en":
            lines.append(f"[HAND] {' | '.join(hand_strs)}")
        else:
            lines.append(f"{_section('hand', lang)} {' | '.join(hand_strs)}")
        if selected_slots:
            sel_cards = [available[i] for i in sorted(selected_slots) if i < len(available)]
            sel_str = ' '.join(_card_str(c, lang=lang) for c in sel_cards)
            if lang == "en":
                lines.append(f"[SELECTED] {sel_str} ({len(sel_cards)} cards)")
            else:
                lines.append(
                    f"{_section('selected', lang)} {sel_str} ({len(sel_cards)})"
                )

    # Jokers
    jokers = d.get("jokers", [])
    if jokers:
        joker_strs = ' | '.join(_joker_str(j, lang=lang) for j in jokers)
        if lang == "en":
            lines.append(f"[JOKERS] {joker_strs}")
        else:
            lines.append(f"{_section('jokers', lang)} {joker_strs}")

    # Consumables
    consumables = d.get("consumables", [])
    if consumables:
        cons_strs = ' | '.join(_consumable_str(c, lang=lang) for c in consumables)
        if lang == "en":
            lines.append(f"[CONSUMABLES] {cons_strs}")
        else:
            lines.append(f"{_section('consumables', lang)} {cons_strs}")

    # Shop (only in shop phase)
    if stage == "Stage_Shop":
        shop_jokers = d.get("shop_jokers", [])
        shop_cons = d.get("shop_consumables", [])
        if shop_jokers:
            items = [
                f"{_joker_str(j, lang=lang)}(${j.get('buy_cost', j.get('cost', '?'))})"
                if isinstance(j, dict)
                else f"{_joker_str(j, lang=lang)}"
                for j in shop_jokers
            ]
            if lang == "en":
                lines.append(f"[SHOP JOKERS] {' | '.join(items)}")
            else:
                lines.append(f"{_section('shop_jokers', lang)} {' | '.join(items)}")
        if shop_cons:
            items = [f"{_consumable_str(c, lang=lang)}" for c in shop_cons]
            if lang == "en":
                lines.append(f"[SHOP CONSUMABLES] {' | '.join(items)}")
            else:
                lines.append(f"{_section('shop_consumables', lang)} {' | '.join(items)}")
        reroll_cost = d.get("shop_reroll_cost", 5)
        if lang == "en":
            lines.append(f"[REROLL COST] ${reroll_cost}")
        else:
            lines.append(f"{_section('reroll_cost', lang)} ${reroll_cost}")

    # Deck info
    deck = d.get("deck", [])
    discarded = d.get("discarded", [])
    if lang == "en":
        lines.append(f"[DECK] {len(deck)} cards remaining | {len(discarded)} discarded")
    else:
        lines.append(
            f"{_section('deck', lang)} "
            f"{len(deck)} / {len(discarded)} ({_label('discards', lang=lang)})"
        )

    # Hand levels (if any upgraded)
    hand_levels = d.get("hand_levels", {})
    upgraded = {k: v for k, v in hand_levels.items() if v > 1}
    if upgraded:
        if lang == "en":
            lvl_strs = [f"{k}: Lv{v}" for k, v in sorted(upgraded.items())]
            lines.append(f"[HAND LEVELS] {', '.join(lvl_strs)}")
        else:
            lvl_strs = [
                f"{_tr('hand_types', k, lang=lang, default=k)}: Lv{v}"
                for k, v in sorted(upgraded.items())
            ]
            lines.append(f"{_section('hand_levels', lang)} {', '.join(lvl_strs)}")

    # Legal actions
    if legal_actions:
        if lang == "en":
            lines.append(f"[LEGAL ACTIONS] {', '.join(legal_actions)}")
        else:
            lines.append(f"{_section('legal_actions', lang)} {', '.join(legal_actions)}")

    return "\n".join(lines)


def load_rules_guide() -> str:
    """Load the Balatro rules guide for LLM context."""
    guide_path = Path(__file__).resolve().parents[1] / "rules" / "balatro_guide_for_llm.md"
    if guide_path.exists():
        return guide_path.read_text()
    return ""


_RULES_CACHE: str | None = None


def get_rules_guide() -> str:
    """Cached rules guide loader."""
    global _RULES_CACHE
    if _RULES_CACHE is None:
        _RULES_CACHE = load_rules_guide()
    return _RULES_CACHE


def serialize_for_llm_prompt(snapshot: Any, legal_actions: list[str],
                              include_rules: bool = False,
                              lang: str = "en") -> str:
    """Create a full LLM prompt with state + instruction.

    Args:
        snapshot: Game state snapshot
        legal_actions: List of legal action names
        include_rules: If True, prepend the full rules guide (for first turn or new games)
        lang: ``"en"`` (default, byte-identical) or ``"zh"``.

    Returns a prompt string ready to send to an LLM.
    """
    state_text = serialize_state(snapshot, legal_actions, lang=lang)

    rules_section = ""
    if include_rules:
        rules = get_rules_guide()
        if rules:
            rules_section = f"""## 游戏规则参考

{rules}

---

"""

    prompt = f"""{rules_section}## 当前局面

{state_text}

## 请决策

分析当前局面并选择最佳动作。用中文思考：
1. 当前局势如何？（阶段、得分进度、资源）
2. 手牌能组成什么牌型？预估得分多少？
3. 应该出牌、弃牌还是其他操作？为什么？
4. 关键提醒：plays=0时绝不弃牌；优先保证$5倍数利息；X Mult小丑最优先购买。

然后用以下格式输出你的选择：
ACTION: <动作名称>

从合法动作列表中选一个。"""

    return prompt
