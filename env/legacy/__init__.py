import warnings as _warnings

_warnings.warn(
    "env.legacy modules (BalatroEnv / state_encoder / action_space / "
    "training / PPO scripts) are deprecated as of 2026-04-24. The primary "
    "interface is balatro_native.Engine + env.state_serializer + "
    "env.canonical_trajectory. See todo/20260424_interface_consolidation_plan.md.",
    DeprecationWarning,
    stacklevel=2,
)
