# mergegate for iOS

The first five slices of [iOS Mobile](../iOS%20Mobile.md): connect through the installed Tailscale app, reopen the selected hub, triage its Decision desk across projects and item states, start a review with a real unified diff, read surrounding source context, and render hub-provided syntax highlighting. iOS 26+, Swift 6, SwiftUI/Observation, and Tuist 4.206.0. There are no package dependencies.

## Build and run

Install Xcode 26.2+ and Tuist (the version is pinned in `mise.toml`). With mise, run `mise install` from this directory, then `tuist generate --no-open`. Open `Mergegate.xcworkspace`, select the Mergegate scheme and an iPhone/iPad simulator, and run. For a physical device, select your signing team on the app target in Xcode. Generated projects and build outputs are ignored.

On the hub machine run `mg hub set listen tailnet`. Enable the installed Tailscale app on the phone, then enter the URL the command prints. Short MagicDNS names, full `.ts.net` names, and Tailscale IPv4/IPv6 addresses are accepted. The hub must admit the caller through `/api/whoami`; the app cannot allow itself. Denied/unknown devices receive the hub's error and recovery hint. Only a successful connection is saved. Hub → Forget hub clears it.

The inbox refreshes on entry, after returning from a review, on pull to refresh/retry, and when foregrounded while visible. A failed refresh leaves the last response visible with an error. Reopening a saved hub revalidates device trust before displaying its inbox. The native large-title navigation bar collapses as the inbox scrolls. Its project menu, All/Decide/Review chips, floating state bar, and detail controls use iOS 26 Liquid Glass; review content stays on solid surfaces. Native materials follow the system’s Reduce Transparency and Reduce Motion settings. The project menu and action chips keep decisions/questions ahead of reviewable work; other states are available in the footer. Decision cards show the pending question or disputed comment, with the full text in a read-only native sheet; working, merging, and closed-today items follow. Visible open rows load the real pending question, disputed comment and rejection reason, or comment awaiting a vote, plus unviewed review-file counts (trusted files are excluded). Each detail response must match the inbox revision; a mismatch asks for an inbox refresh. Per-item failures have their own retry. Persistent offline caching, SSE, and decision/review mutations belong to later slices.

Every inbox item opens its current authoritative review. The screen shows the item identity, revision and target, all-changes comparison scope, and remaining review-file count. Guide opens the complete submitted guide; Files preserves the hub’s importance order and shows its reading reasons, viewed state, and trusted tier. The first review-tier file opens automatically (or the first trusted file when there are no review-tier files). The native TextKit 2 reader displays unified hunks with a compact line-number gutter (old coordinates for removals, new coordinates otherwise), explicit addition/removal markers, and shared semantic colors. Wrapped continuations align with the source column; all blocks in a file share its gutter width. Hunk headers retain both source ranges. Binary, oversized, discarded, and metadata-only diffs have explicit states. Opening or reading never marks a file viewed. The displayed snapshot stays fixed while open, including across foregrounding; returning to the inbox and reopening loads the current revision. The entire review scrolls at accessibility text sizes.

Load unchanged context fetches the comparison’s immutable old/new blobs, then exposes leading, between-hunk, and trailing gaps in batches of 20 lines. A failed expansion leaves the diff readable and can be retried. Renamed and metadata-only text files can expose their unchanged source too. Long lines wrap at the reader width; native selection and Copy preserve source text and Unicode while excluding gutters, change markers, hunk headers, and no-newline notes. Selection spans one displayed text block; expanding gaps creates separate blocks. Viewed-state changes are a later slice.

The reader applies hub-provided UTF-16 syntax spans in source coordinates before wrapping, using the same generated light/dark palette as the web. Removed lines use old-blob spans; additions and context use new-blob spans. The text, diff backgrounds, gutters, and source-only Copy remain intact. Missing grammars, pending spans, failed highlighting requests, unknown tags, and invalid ranges leave readable plain source. One shared foreground SSE transport refreshes highlighting on connect/reconnect and `highlights.changed`; it stops when there are no consumers or the app leaves the foreground. Each file subscription refreshes initially, including when reusing a stream. Highlight refreshes use the displayed immutable blob IDs and never switch the open revision, file, expanded context, or selection. Other live review/inbox updates belong to a later slice.

## Verification

From the repository root:

```sh
.claude/scripts/check.sh lint ios
.claude/scripts/check.sh test ios
```

`lint ios` generates and builds; `test ios` runs Swift Testing behavior tests and XCTest UI tests. `MG_IOS_DESTINATION` overrides the default iPhone 17 Pro / latest simulator destination. Logs and xcresult bundles are under `ios/.build/`.

The UI flow requires a real disposable hub and review repository. It skips when `MG_IOS_TEST_HUB_URL` is absent. The app has no stub transport or testing bypass. With Tailscale running on the Mac:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-smoke
# Use the tailnet URL printed under listening.tailnet.url:
MG_IOS_TEST_HUB_URL=http://my-mac:PORT .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-smoke
```

Choose a fresh directory on each run. The fixture uses the installed `mg` (override with `MG_BIN`), an isolated `MERGEGATE_HOME`, the real Tailscale identity/trust checks, and a local git backend. It does not touch the operator's hub or create GitHub PRs. The UI test enters the URL, checks the submitted item, audits contrast/Dynamic Type/touch targets/accessibility labels, terminates and reopens the app, verifies the saved URL, then forgets it. Screenshots are kept in the xcresult for review. The simulator uses the Mac's installed Tailscale connection; a physical iPhone uses its own installed Tailscale app.

For the triage UI flow, create a fresh smoke hub, seed its disposable repositories, and run:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-triage
python3 ios/Scripts/triage-fixture.py /tmp/mergegate-ios-triage
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_TRIAGE=1 .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-triage
```

The additional fixture uses real review routes on the isolated smoke hub to seed a sent question, one viewed file out of two, working/closed items, and a second project. It never contacts the operator hub. The UI test verifies large-title collapse/expansion, project/state and action filtering, full decision text, details surviving foreground refresh, empty results, remaining-work counts, accessibility, and relaunch loading, and retains screenshots. Pending external disagreements/votes and answered/agreed threads are covered by Swift contract tests; the local git fixture has no GitHub comments.

For the start-review UI flow, use a fresh smoke hub and seed its guided, importance-ordered change:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-review
python3 ios/Scripts/review-fixture.py /tmp/mergegate-ios-review
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_REVIEW=1 .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-review
```

This flow opens a real inbox item, checks revision/target/scope, reads its guide and ordered file reasons, verifies source text and addition/removal coordinates, opens a binary file, and returns/reopens. It audits contrast, Dynamic Type, touch targets, and labels, reaches the end of a long diff, resets position when changing files, and opens the review at the largest accessibility text size. It retains screenshots in the xcresult. Pixel goldens cover native attributed diff rendering in light, dark, and accessibility text sizes. Swift behavior tests cover failed-request retry, mismatched identity/comparison, content boundaries, empty diff sides, no-newline markers, Unicode prefixes, and source-coordinate mapping.

For the source-context UI flow, create a fresh smoke hub and seed its two-hunk comparison:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-source
python3 ios/Scripts/source-fixture.py /tmp/mergegate-ios-source
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_SOURCE=1 .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-source
```

This flow loads immutable blobs through the real hub, expands leading/between/trailing context, reads a long wrapped Unicode line, opens native selection/Copy, switches to a metadata-only rename, and checks dark mode at the largest accessibility size. It audits contrast, Dynamic Type, touch targets, and labels and retains screenshots. Behavior tests verify source coordinates across insertions/deletions, empty sides, invalid gaps, retry, exact UTF-16 copy ranges, and missing final newlines.

For the highlighted-code UI flow, package an already installed Swift grammar and seed a fresh smoke hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-highlight
python3 ios/Scripts/highlight-fixture.py /tmp/mergegate-ios-highlight --pack ~/.mergegate/languages/swift/0.7.1
MG_IOS_TEST_HUB_URL=http://my-mac:PORT \
  MG_IOS_TEST_HIGHLIGHT=/private/tmp/mergegate-ios-highlight/swift.tar.gz \
  .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-highlight
```

Use the installed pack's actual version directory. The test opens plain source on the isolated hub, expands Unicode context, installs the real pack through that hub, and verifies SSE repainting without changing text, source layout, revision, or expanded context. It audits accessibility and retains normal/light and dark/accessibility screenshots. Unit tests cover span tuples, side mapping, malformed UTF-16 ranges, unknown categories, styling traits, failed requests, revision pinning, and raw SSE byte framing; pixel goldens cover highlighted light/dark/accessibility rendering and source-only Copy.

## Module boundaries

- `App` composes feature builders.
- `HubConnection` owns URL validation, `/api/whoami`, connection presentation, and the selected URL in UserDefaults. `HubConnectionInterface` exposes only the established connection value needed by Inbox.
- `Inbox` owns `/api/inbox` and its consumed `/api/items/{id}` triage subset, project/state filtering, pending-decision interpretation, refresh policy, state, and rows.
- `Review` owns its `/api/items/{id}?since=all` subset, guide/file presentation, snapshot loading/retry, selected file, and loading/retrying blobs and highlights from the displayed revision, and its highlighting event policy. Inbox accepts a destination builder; App composes it with Review without sharing DTOs.
- `DiffDocument` maps unified hunks and unchanged blob gaps into source-coordinate lines; `DiffDocumentInterface` exposes only those rendering values.
- `CodeStyling` implements `CodeStylingInterface`, mapping neutral UTF-16 spans to native attributes; App injects the implementation into Review and DiffView.
- `DiffView` renders those values with UIKit/TextKit 2, native selection with source-only Copy, wrapping, and feature-local generated appearance.
- `Networking` provides transport primitives: bounded URLSession requests, hub error decoding, and rejection of redirects, and a multiplexed reconnecting SSE byte stream. It owns no endpoint or feature refresh policy.
- `Appearance/Appearance.swift` is generated from [shared tokens](../appearance/README.md) and compiled privately into features. DM Sans is registered by the app; semantic colors follow light/dark appearance and typography scales with Dynamic Type.

The Rust files `crates/api/src/device.rs`, `item.rs`, `diff.rs`, and `review.rs` remain the wire authority. Feature DTOs intentionally decode their consumed fields and ignore additional fields; the real-hub UI flow exercises the contract.
