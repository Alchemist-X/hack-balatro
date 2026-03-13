from __future__ import annotations

import json
from pathlib import Path


def test_generated_ruleset_fixture_counts() -> None:
    bundle_path = Path("fixtures/ruleset/balatro-1.0.1o-full.json")
    assert bundle_path.exists()

    bundle = json.loads(bundle_path.read_text())
    assert bundle["metadata"]["version"] == "1.0.1o-FULL"
    assert len(bundle["jokers"]) == 150
    assert len(bundle["blinds"]) == 30
    assert len(bundle["stakes"]) == 8
    assert len(bundle["ante_base_scores"]) == 8
    assert "resources/textures/1x/Jokers.png" in bundle["sprite_manifest"]
