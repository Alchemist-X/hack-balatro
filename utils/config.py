from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from typing import Any

import yaml


class ConfigError(RuntimeError):
    pass


def load_yaml(path: str | Path) -> dict[str, Any]:
    config_path = Path(path)
    if not config_path.exists():
        raise ConfigError(f"Missing config file: {config_path}")
    with config_path.open("r", encoding="utf-8") as f:
        raw = yaml.safe_load(f) or {}
    if not isinstance(raw, dict):
        raise ConfigError("Root config must be a mapping")
    return raw


def deep_merge(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    merged = deepcopy(base)
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(merged.get(key), dict):
            merged[key] = deep_merge(merged[key], value)
        else:
            merged[key] = deepcopy(value)
    return merged


def with_strategy(config: dict[str, Any], strategy_name: str | None = None) -> dict[str, Any]:
    strategy_root = config.get("strategy", {})
    strategies = strategy_root.get("strategies", {})

    if strategy_name is None:
        strategy_name = strategy_root.get("default")
    if not strategy_name:
        return config

    if strategy_name not in strategies:
        available = ", ".join(sorted(strategies.keys()))
        raise ConfigError(f"Unknown strategy '{strategy_name}'. Available: {available}")

    strategy_cfg = strategies[strategy_name]
    merged = deep_merge(config, {
        "selected_strategy": strategy_name,
        "model": {"type": strategy_cfg.get("model_type", "mlp")},
        "training": strategy_cfg.get("training", {}),
        "reward": strategy_cfg.get("reward", {}),
    })

    optimizer_cfg = strategy_cfg.get("optimizer", {})
    if optimizer_cfg:
        merged.setdefault("training", {})
        if "learning_rate" in optimizer_cfg:
            merged["training"]["learning_rate"] = optimizer_cfg["learning_rate"]
        if "scheduler" in optimizer_cfg:
            merged["training"]["scheduler"] = optimizer_cfg["scheduler"]

    return merged
