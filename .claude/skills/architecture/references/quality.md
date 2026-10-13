# Performance and testing

## Budgets (enforced in CI)

| Path | Budget |
|---|---|
| `mg` cold start (`mg --version`) | < 10 ms |
| `mg inbox --json` | < 30 ms |
| UI first paint | < 100 ms |
| Open item (precomputed) | < 50 ms |
| Switch file / keyboard action | < 16 ms |
| Scroll a 5k-line diff | 60 fps (virtualized) |

The UI prefetches the next item and next file, and updates optimistically (marking viewed and similar actions don't wait for the server).

## Testing

- Real git repos in temp dirs for git, diff, merge and anchoring tests.
- Golden files for diffs, interdiffs, highlighting spans and triage output.
- A fake GitHub server with recorded fixtures for the backend.
- Playwright end-to-end tests against a hub with a fixture database.
- Benchmarks for every budget above; a regression fails the build.
