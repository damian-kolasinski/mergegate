# 15 — Highlighting

Roadmap row 15 · decisions settled 2026-10-14

## Scope

Diffs on the review screen are syntax highlighted by the hub, with grammars the operator installs as language packs (tech-stack.md §6). Ships `mg lang list`, `mg lang add --from`, `mg lang remove`, the packs' build in CI, highlighting of open items in the background, and highlighted hunks and expanded context in the UI. No flow in user-flows.html covers it.

## Decisions

1. **The hub highlights with tree-sitter,** grammars loaded as WASM through wasmtime; §11's "Shiki in a worker" is dropped. The UI paints the hub's spans into CodeView by wrapping its renderer's highlight step, the one place that sees each line's side and number.
2. **Packs install from a file only:** `mg lang add --from PATH` (a pack, or a directory of them), `mg lang list`, `mg lang remove`. Downloading by name, `--available`, `--detect`, `update` and detection in `mg project add` need release assets, so they move to PR 18.
3. **The first packs:** rust, typescript (with tsx), javascript, json, toml, markdown (with its inline grammar), css, html, bash, yaml, python, go, swift, kotlin, elixir. Injections work across installed packs (fenced code in Markdown, `<script>` in HTML).
4. **Nothing is bundled.** `mg` ships with no grammar; a file without an installed pack renders plain, with a hint naming the pack.
5. *(agent)* **Packs are built from pinned npm releases,** compiled with tree-sitter CLI 0.27.1 (`tree-sitter build --wasm`, wasi-sdk), never taken prebuilt, so every grammar has the runtime's ABI. `langs/packs.json` pins each package by version and integrity; `langs/build.mjs` writes one `<name>-<version>.tar.gz` each, and CI uploads them as the `language-packs` artifact after loading every one.
6. *(agent)* **A pack** is a gzipped tar of `pack.json` (name, version, grammars with their file extensions, names and query files) and per grammar its `.wasm` and queries. The hub keeps it in `languages/<name>/<version>/`; adding a pack replaces its other versions.
7. *(agent)* **Highlighting runs in the background,** not in the submit request: a hub task highlights the blobs of every open item's newest revision that lack spans for an installed grammar, woken by submits and `mg lang add`. A file opened before its spans exist shows plain until the next load. Request paths only read.
8. *(agent)* **Spans are cached in SQLite** per `(blob, grammar, grammar version)` as `[start, len, class]` per line, with columns in UTF-16 units, as the browser counts them. They're pruned with the diffs when an item closes, and dropped for a pack that's removed or replaced.
9. *(agent)* **Hunks carry their lines' spans** in `GET /api/items/{id}`, so opening an item needs no extra request; expanding a gap reads the whole blob's spans from `GET /api/highlights/{grammar}/{blob}` along with the blob.
10. *(agent)* **Classes are a fixed set** mapped from tree-sitter capture names (keyword, string, comment, function, type, …), coloured with Pierre's light and dark theme colours so highlighted and plain views match.

## Where it lives

- `core::highlight`: the `Highlighter` port, the `HighlightStore` cache, grammar lookup by path, the background pass, and spans for an item's files.
- New `crates/highlight` adapter: packs (install, remove, list), wasmtime and tree-sitter-highlight.
- `store-sqlite`: `0002_highlights.sql`, `highlights.rs`.
- `api::lang`, spans on `api::diff`; `hub::lang`, `hub::highlight`; `cli::lang_cmd`.
- UI: `ui/diff-view/highlight.ts` and the classes' colours.
- `langs/packs.json`, `langs/build.mjs`; a `langs` job in CI.

## Stack

- `highlighting`: everything above, one PR. *In progress.*

## Open

- **PR 18:** downloads by name, `--available`, `--detect`, `update`, pinned checksums, detection in `mg project add`, and `mg doctor` flagging packs whose ABI the new `mg` can't load.
- **Doc drift:** tech-stack.md §3 (diff view) and §11 (both highlighting questions), §6 for the decisions above.
