# Licensed Source Bootstrap

## Purpose

This repo does not commit or push the commercial Balatro package or the extracted Lua source.
Instead, each licensed teammate reproduces the same local mirror from their own installed copy.

## Default macOS source path

```text
~/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love
```

## Bootstrap command

```bash
python scripts/bootstrap_balatro_source.py
```

This copies the local `Balatro.love` package into the repo's ignored vendor directory:

```text
vendor/balatro/steam-local/original/Balatro.love
vendor/balatro/steam-local/extracted/
vendor/balatro/steam-local/manifest.json
```

## Expected package hash

The current team baseline is:

```text
SHA-256: 48c7a0791796a969d2cd0891ebdc9922b2988eb5aaad8ad7a72775a02772e24e
```

If your local install hashes differently, stop and verify the game version before generating new rulesets or trajectories.

## Custom source path

```bash
python scripts/bootstrap_balatro_source.py --source "/path/to/Balatro.love"
```

## Policy

- Do not commit `Balatro.love` or extracted Lua files to git.
- Do not remove the `vendor/` ignore rule.
- Only commit structured outputs, hashes, manifests, and reverse-engineering notes.
