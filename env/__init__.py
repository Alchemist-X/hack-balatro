"""Primary interface modules.

Historical Gym/PPO wrapper lives in ``env.legacy``; it is deprecated as
of 2026-04-24. See ``todo/20260424_interface_consolidation_plan.md`` for
the decision + revival recipe.
"""
# Intentionally no auto-exports; consumers should import from specific
# submodules (state_serializer, locale, canonical_trajectory, state_mapping).
