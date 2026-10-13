# 4 · UI foundation and inbox

## Scope

The hub serves a SolidJS UI at `/` with the design system and the inbox screen, updated live. Covers the inbox part of F6 ([user-flows.html](../user-flows.html)): open the hub, see open items grouped as in design.md §9.1, move with `j/k`, open with `enter`. Playwright runs against the real hub.

## Decisions

Settled with the operator:

1. **Build:** `crates/hub/build.rs` runs `pnpm install --frozen-lockfile && pnpm build` into `OUT_DIR` and the hub embeds the result. Without pnpm it embeds a stub page and warns; `MG_REQUIRE_UI=1` turns that into an error. Nothing built is committed.
2. **Bundler:** esbuild with `esbuild-plugin-solid` (Solid's JSX needs its Babel transform).
3. **Wire types:** ts-rs on `api` behind a `ts` feature; `build.rs` exports them into gitignored `ui/api/` before bundling, `pnpm gen` does the same for editors.
4. **Live updates:** SSE at `GET /api/events/ui` carrying a bare `inbox.changed`; PR 6 puts the event log on it.
5. **Inbox fields:** only what `GET /api/inbox` has today: id, title, revision, files, +/−, age, grouped as needs you / with agents / merging. Later PRs add their own columns.
6. **Keys:** `j/k` move, `enter` opens `#/items/<id>`, a stub until PR 5.
7. **Look:** colours and type from the [Academy Figma](https://www.figma.com/design/KiTRYqOIJ3I30j1VjB1156/Academy?node-id=0-1). Light and dark follow the OS.
8. **Framework:** SolidJS, linted by ESLint with `eslint-plugin-solid` as errors; Biome only formats.
9. **Browsers:** Playwright runs Chromium only (the operator uses Arc and Chrome).

Taken by the agent:

10. `ui/design-system/tokens.css` holds the Figma ramps as private `--ramp-*` properties and the semantic tokens (`--text-primary`, `--background-page`, …) as the only ones features use, with Figma's names. Copied by hand, with the Figma node id at the top; no sync script.
11. DM Sans is self-hosted as one variable woff2 (latin subset), preloaded, `font-display: swap`. Code and ids use the system `ui-monospace` stack; Figma defines no mono.
12. The dense screens use Figma's small styles: p3 body, p4 meta, label3 chips, h4/h5 headings.
13. Tokens Figma lacks are derived from its ramps and marked as such: brand fill and row selection (blue 50/900), focus ring (`border-brand`), spacing (4 px steps), radius.
14. Assets keep plain names, served with ETag and `no-cache`; no precompression on loopback. Hash routing, so `/api` and `/git` stay as they are.
15. `mg doctor` reports a hub built with the stub page; `.agents/prompts/install.md` sets `MG_REQUIRE_UI=1`.
16. First paint under 100 ms (tech-stack.md §9), measured by Playwright, enforced on the Ubuntu CI job.
17. Figma's `background-error/success/warning` are solid badge fills with `background-page` text (≥ 5.7:1 in both themes); tinted rows use the ramp's 50 step with the matching `text-*` (≥ 7.3:1).

## Where it lives

- `crates/hub`: `build.rs`, a `ui` route module serving the embedded files, an `events` route module for SSE, a broadcast sender in `AppState` that `items` fires on submit.
- `crates/api`: optional `ts` feature deriving ts-rs.
- `crates/cli`: doctor check for the stub UI.
- `ui/`: `package.json`, `app/` (shell, router, keyboard map), `design-system/` (tokens, fonts, primitives), `inbox/`, `tests/` (Playwright, `globalSetup` runs `mg` with a temporary `MERGEGATE_HOME`).
- CI: Node and pnpm in the test job, a UI job with `MG_REQUIRE_UI=1` running lint, typecheck and Playwright; `check.sh` runs the UI checks when pnpm is present.
- No migrations.

## Stack

- 4a `ui-1-build`: build.rs, embedding, stub page, `MG_REQUIRE_UI`, doctor check, ts-rs export, CI and check.sh, this brief. Planned.
- 4b `ui-2-design-system`: tokens, fonts, primitives, keyboard map. Planned.
- 4c `ui-3-inbox`: inbox screen, SSE, Playwright scenarios and first-paint budget. Planned.

## Open

- tech-stack.md §3: name pnpm and the Solid plugin next to esbuild.
- Roadmap PR 18 row: drop `## Stage` from AGENTS.md.
