"""Runtime locale lookup for Balatro IDs and UI labels.

Default language is English: lookups pass identifiers through unchanged so
existing LLM-training pipelines stay byte-identical. Chinese is opt-in via
``load_locale("zh")`` (or passing ``lang="zh"`` to ``name`` / ``label``) and
returns zh_CN names from ``fixtures/locale/zh_CN.json``.

The JSON file is committed so we never reparse the Lua file at runtime.

Examples:
    >>> name("jokers", "j_joker", lang="en")
    'j_joker'
    >>> name("jokers", "j_joker", lang="zh")
    '小丑'
    >>> name("jokers", "j_not_real", lang="zh", default="Fallback")
    'Fallback'
    >>> label("stage", lang="en")
    'STAGE'
    >>> label("stage", lang="zh")
    '阶段'
"""
from __future__ import annotations

import json
from pathlib import Path
from typing import Literal

Lang = Literal["en", "zh"]

_LOCALE_DIR = Path(__file__).resolve().parents[1] / "fixtures" / "locale"

_cache: dict[str, dict] = {}


def load_locale(lang: Lang | str) -> dict:
    """Load (and cache) the locale dict for ``lang``. English is empty."""
    if lang in _cache:
        return _cache[lang]
    if lang == "en":
        _cache["en"] = {}
        return _cache["en"]

    # Support both "zh" (-> zh_CN.json) and explicit "zh_CN".
    candidates = [
        _LOCALE_DIR / f"{lang}.json",
        _LOCALE_DIR / f"{lang}_CN.json",
    ]
    if lang == "zh":
        candidates = [_LOCALE_DIR / "zh_CN.json"] + candidates

    for path in candidates:
        if path.exists():
            _cache[lang] = json.loads(path.read_text(encoding="utf-8"))
            return _cache[lang]

    raise FileNotFoundError(
        f"No locale file for lang={lang!r}; tried {[str(p) for p in candidates]}"
    )


def name(
    kind: str,
    id_: str,
    *,
    lang: Lang | str = "en",
    default: str | None = None,
) -> str:
    """Translate an ID to its display name.

    ``kind`` is one of ``jokers``, ``tags``, ``blinds``, ``hand_types``,
    ``consumables``, ``decks``, ``ranks``, ``suits``, ``vouchers``, ``planets``,
    ``tarots``, ``spectrals``. English always returns ``id_`` unchanged (or
    ``default`` if supplied). Chinese returns the mapped string, falling back
    to ``default`` or ``id_`` if the entry is missing.
    """
    if lang == "en":
        return id_ if default is None else default
    locale = load_locale(lang)
    return locale.get(kind, {}).get(id_, default if default is not None else id_)


def label(key: str, *, lang: Lang | str = "en") -> str:
    """Translate a section-header key to its display form.

    English returns ``KEY`` (uppercased, underscores -> spaces) so the existing
    ``[STAGE]`` / ``[LEGAL ACTIONS]`` style is preserved.
    """
    if lang == "en":
        return key.upper().replace("_", " ")
    locale = load_locale(lang)
    return locale.get("labels", {}).get(key, key.upper())


def available_languages() -> list[str]:
    """List locale codes we have JSON files for."""
    if not _LOCALE_DIR.exists():
        return ["en"]
    langs = ["en"]
    for p in sorted(_LOCALE_DIR.glob("*.json")):
        stem = p.stem
        # Accept zh_CN or zh
        langs.append(stem)
    return langs
