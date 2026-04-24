"""Prompt assembly for LLM agents.

Takes pure factual state output from :mod:`env.state_serializer` and wraps
it with a neutral prompt template suitable for sending to a language model.

No strategy hints live here — per ``CLAUDE.md`` "Objective vs Subjective —
Content Rule (MANDATORY)", opinions belong in per-agent playbooks (e.g.
``agents/<name>/playbook.md``), not in model-global framework code.

If a caller wants an opinionated system prompt, pass ``system_prompt=``
explicitly; the default is deliberately strategy-free.

Usage::

    from env.prompt_builder import build_prompt
    prompt = build_prompt(snapshot, legal_actions)            # English default
    prompt = build_prompt(snapshot, legal_actions, lang="zh") # Chinese labels
    prompt = build_prompt(snapshot, legal_actions,
                          system_prompt=my_agent_playbook_text)

See ``docs/archived_strategy_hints_20260424.md`` for the strategy text
that was previously baked in and removed on 2026-04-24.
"""
from __future__ import annotations

from typing import Any

from env.locale import Lang
from env.state_serializer import get_rules_guide, serialize_state


DEFAULT_SYSTEM_EN = (
    "You are playing Balatro. You will see the current game state and a list "
    "of legal actions. Respond with a single legal action identifier on a line "
    "prefixed with `ACTION:` (for example `ACTION: play` or "
    "`ACTION: buy_shop_item_0`). You may optionally include brief reasoning on "
    "earlier lines."
)

DEFAULT_SYSTEM_ZH = (
    "你正在玩 Balatro。你会看到当前游戏状态和一个合法动作列表。"
    "请在一行上以 `ACTION:` 开头回复一个合法动作标识符"
    "（例如 `ACTION: play` 或 `ACTION: buy_shop_item_0`）。"
    "你可以在前面的行里写简短的推理（可选）。"
)


def _default_system_prompt(lang: str) -> str:
    return DEFAULT_SYSTEM_EN if lang == "en" else DEFAULT_SYSTEM_ZH


def build_prompt(
    snapshot: Any,
    legal_actions: list[str] | None = None,
    *,
    lang: Lang | str = "en",
    system_prompt: str | None = None,
    include_rules: bool = False,
) -> str:
    """Return a full prompt string ready for LLM consumption.

    Args:
        snapshot: Engine snapshot (PySnapshot or dict).
        legal_actions: List of legal action names to show the model.
        lang: ``"en"`` (default) or ``"zh"`` for locale-translated state
            labels. Only affects the factual state section unless
            ``system_prompt`` is ``None``, in which case the default
            system prompt is also localized.
        system_prompt: Optional override for the system framing. If
            ``None``, a neutral default is used (no strategy). Callers
            that want opinionated strategy must source their own string
            from an agent playbook and pass it here explicitly.
        include_rules: If ``True``, prepend the full Balatro rules guide
            (useful for first-turn / cold-start prompts).

    Returns:
        A multi-section prompt string, safe to send as a single user
        message. Sections are separated by blank lines.
    """
    sp = system_prompt if system_prompt is not None else _default_system_prompt(lang)
    body = serialize_state(snapshot, legal_actions, lang=lang)

    sections: list[str] = [sp]
    if include_rules:
        rules = get_rules_guide()
        if rules:
            sections.append(rules)
    sections.append(body)

    return "\n\n".join(sections)
