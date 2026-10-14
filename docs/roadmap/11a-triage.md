# 11a — File triage

Roadmap row 11a · decisions settled 2026-10-14

## Scope

Every changed file lands in `review` or `trusted` (design.md §7.1–7.3) by the operator's rules in `~/.mergegate/projects/<p>/rules.toml` and the rule-file guard. Trusted files fold away on the review screen, and the inbox counts only review-tier lines. Agents propose rules when the operator asks, and a looser rule waits for approval in the UI with a preview. `mg submit --guide` orders the review files with reasons and fills the Intent panel. That's F12 and F11's guide step. The comment guard is 11b and the judge is 17.

## Decisions

1. **Three PRs:** rules and tiers, then proposals and approval, then the guide (Stack).
2. **Generated** means `linguist-generated` as the item's base tree sets it (`git check-attr --source`, so git ≥ 2.40). **The guard** keeps any changed `.gitattributes` in `review`, since it can turn that flag on.
3. **Matchers:** `path` (glob, `*` stays within a directory), `kind` (`added`, `deleted`, `renamed`, `generated`, `modified`), `max_lines` (added + removed), `diff_matches` (regex over added and removed lines). A rule matches when all its matchers do. First match wins, and unmatched files are `review`. When a matcher can't be sure (a rename's two paths, a binary or too-large diff) it picks whichever answer doesn't hide the file.
4. **A proposed `review` rule goes first, a `trusted` one last.** Reordering is a hand edit.
5. **Approval happens only in the UI.** Agents propose when the operator asks, never on their own; `mg rules propose` prints the rule's ID and whether it applied or waits, and `mg rules show ID` says which later.
6. **A hand edit that loosens waits too.** The hub keeps the last approved rules and watches the file. An edit that only adds `review` rules or removes `trusted` ones applies; anything else (adds `trusted`, removes `review`, reorders) waits as one pending change, which approve accepts and reject reverts in the file. An invalid file is ignored, and its error shows in the UI and in `mg rules list`.
7. **The preview** runs once, when a proposal arrives, over the project's open items and its last 20 merged ones: "would have hidden N files in M items", with the list.
8. **Trusted files** fold into a collapsed "Trusted (N)" group, out of the diff scroll until it opens. They count as viewed without a stored mark, and each says which rule hid it ("trusted by R3: `glob` — reason").
9. **The inbox and `mg inbox`** show the review-tier files next to all of them ("4 of 6 files"), and the lines of all of them, +A −D.
10. **The guide is Markdown;** its `## Files` list of `` - `path` — reason `` bullets orders the review files and gives reasons. The other review files follow in path order. Unknown paths come back as warnings, and a submit without `--guide` keeps the last one. The guide can't hide or promote a file.
11. **The Intent panel** at the top of the review screen shows the guide as plain text.
12. *(agent)* **`core::triage`** holds rules, matching, the loosening check and the guide parser, pure. A `RulesFile` port reads and writes `rules.toml`, adapted by a new `rules-toml` crate.
13. *(agent)* **Rule IDs** are `R<n>`, kept in the file and never reused. A hand-added rule needs one.
14. *(agent)* **Tiers are stored per revision file** with the rule that set them, and the generated flag beside them. When the approved rules change, every open item's revisions are re-triaged from the cached diffs and the UI is pinged.

## Where it lives

- `core`: `triage` (rules, `classify`, `loosens`, `guide`), the `RulesFile` and `RulesStore` ports; `item::submit` triages; `git::MIN_VERSION` goes to 2.40.
- `rules-toml` (new adapter); `git`: `check-attr` on the staging repo.
- `store-sqlite`: `0005_triage.sql` (tier columns, approved rules, proposals).
- `api::rules`; `hub::rules` (routes, the file watch, re-triage); `cli::rules_cmd`; the skill's references.
- UI: tier groups in `ui/item/files.tsx`, the Intent panel, the rules screen and the inbox badge.

## Stack

- `triage-1-rules`: `rules.toml`, matching, the guard, stored tiers and re-triage, the Trusted group, the inbox's count of files to review, `mg rules list`, this brief. *Planned.*
- `triage-2-proposals`: `mg rules propose/show`, pending changes (proposed and hand-edited), approve and reject with the preview, the inbox badge, the skill's operator reference. *Planned.*
- `triage-3-guide`: `mg submit --guide`, ordering and reasons, the Intent panel; F11's guide step and F12 end to end, doc updates, deleting this brief and roadmap row 11a. *Planned.*

## Open

- **Doc drift:** design.md §7.2 says "mergegate rule files" where the guard covers `.gitattributes`; tech-stack.md's git version becomes 2.40.
- **11b** adds the comment guard to `classify`'s guards.
- **17** feeds the judge's ranks and promotions into the same ordering the guide uses.
