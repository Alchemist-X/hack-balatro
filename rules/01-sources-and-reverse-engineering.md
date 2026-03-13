# Sources And Reverse Engineering Rules

## Source precedence

1. Local Steam install:
   - `~/Library/Application Support/Steam/steamapps/common/Balatro/Balatro.app/Contents/Resources/Balatro.love`
   - This is the primary authority for extracted Lua logic and texture atlases.
2. Balatro Wiki:
   - `https://balatrowiki.org/w/Balatro_Wiki`
   - `https://balatrowiki.org/w/Module:Jokers/data?action=raw`
   - `https://balatrowiki.org/w/Guide:_Activation_Sequence`
3. Reference repos for implementation ideas only:
   - `https://github.com/evanofslack/balatro-rs`
   - `https://github.com/cassiusfive/balatro-gym`

## Internet findings for this repo

- Balatro Wiki states that as of **2026-03-13** it is up to date with **`1.0.1o-FULL`**.
- The previously referenced GitHub extraction repo `vibezfire/balatro-game.lua-files` currently returns `404` and must not be treated as a required dependency.
- Because third-party extracted repos are unstable, the local `.love` package is mandatory for ruleset generation.

## Reverse-engineering rules

- Reverse engineering output must be committed as structured data, not hidden in code comments.
- Every extracted ruleset bundle must store:
  - source file paths
  - source hashes
  - generation timestamp
  - sprite atlas mapping
- When local Lua and community docs disagree, local Lua wins and the discrepancy must be logged in `progress.md`.
