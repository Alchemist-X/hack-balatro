# Rough Loop

Each implementation loop must do all of the following:

1. Update `progress.md`.
2. Update `rules/` if the loop changes acceptance criteria, source precedence, or reverse-engineering findings.
3. Leave a checklist with:
   - completed
   - in progress
   - next
   - need human help
4. Generate or refresh at least one visible artifact:
   - replay JSON
   - autoplay replay HTML
   - extracted atlas preview if the visual layer changed

## Output convention

- Replay JSON goes under `results/`.
- Extracted atlases go under `results/assets-preview/`.
- Autoplay replay HTML goes under `results/`.
- File names should prefer `latest` aliases when they are the newest loop artifact.
