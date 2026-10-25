# mergegate for iOS

The first nine slices of [iOS Mobile](../iOS%20Mobile.md): connect through the installed Tailscale app, reopen the selected hub, triage its Decision desk across projects and item states, start a review with a real unified diff, read surrounding source context, render hub-provided syntax highlighting, track viewed files, keep source-anchored comment drafts, follow existing conversations, and send feedback with a summary. iOS 26+, Swift 6, SwiftUI/Observation, and Tuist 4.206.0. There are no package dependencies.

## Build and run

Install Xcode 26.2+ and Tuist (the version is pinned in `mise.toml`). With mise, run `mise install` from this directory, then `tuist generate --no-open`. Open `Mergegate.xcworkspace`, select the Mergegate scheme and an iPhone/iPad simulator, and run. For a physical device, select your signing team on the app target in Xcode. Generated projects and build outputs are ignored.

On the hub machine run `mg hub set listen tailnet`. Enable the installed Tailscale app on the phone, then enter the URL the command prints. Short MagicDNS names, full `.ts.net` names, and Tailscale IPv4/IPv6 addresses are accepted. The hub must admit the caller through `/api/whoami`; the app cannot allow itself. Denied/unknown devices receive the hub's error and recovery hint. Only a successful connection is saved. Hub → Forget hub clears it.

The inbox refreshes on entry, after returning from a review, on pull to refresh/retry, and when foregrounded while visible. A failed refresh leaves the last response visible with an error. Reopening a saved hub revalidates device trust before displaying its inbox. The native large-title navigation bar collapses as the inbox scrolls. Its project menu, All/Decide/Review chips, floating state bar, and detail controls use iOS 26 Liquid Glass; review content stays on solid surfaces. Native materials follow the system’s Reduce Transparency and Reduce Motion settings. The project menu and action chips keep decisions/questions ahead of reviewable work; other states are available in the footer. Decision cards show the pending question or disputed comment, with the full text in a read-only native sheet; working, merging, and closed-today items follow. Visible open rows load the real pending question, disputed comment and rejection reason, or comment awaiting a vote, plus unviewed review-file counts (trusted files are excluded). Each detail response must match the inbox revision; a mismatch asks for an inbox refresh. Per-item failures have their own retry. Persistent offline caching, live review updates, decisions, and approval belong to later slices.

Every inbox item opens its current authoritative review. The screen shows the item identity, revision and target, all-changes comparison scope, and remaining review-file count. Guide opens the complete submitted guide; Files preserves the hub’s importance order and shows its reading reasons, viewed state, and trusted tier. The first review-tier file opens automatically (or the first trusted file when there are no review-tier files). The native TextKit 2 reader displays unified hunks with a compact line-number gutter (old coordinates for removals, new coordinates otherwise), explicit addition/removal markers, and shared semantic colors. Wrapped continuations align with the source column; all blocks in a file share its gutter width. Hunk headers retain both source ranges. Binary, oversized, discarded, and metadata-only diffs have explicit states. Opening or reading never marks a file viewed. The displayed snapshot stays fixed while open, including across foregrounding; returning to the inbox and reopening loads the current revision. The entire review scrolls at accessibility text sizes.

Load unchanged context fetches the comparison’s immutable old/new blobs, then exposes leading, between-hunk, and trailing gaps in batches of 20 lines. A failed expansion leaves the diff readable and can be retried. Renamed and metadata-only text files can expose their unchanged source too. Long lines wrap at the reader width; native selection and Copy preserve source text and Unicode while excluding gutters, change markers, hunk headers, and no-newline notes. Selection spans one displayed text block; expanding gaps creates separate blocks.

The reader applies hub-provided UTF-16 syntax spans in source coordinates before wrapping, using the same generated light/dark palette as the web. Removed lines use old-blob spans; additions and context use new-blob spans. The text, diff backgrounds, gutters, and source-only Copy remain intact. Missing grammars, pending spans, failed highlighting requests, unknown tags, and invalid ranges leave readable plain source. One shared foreground SSE transport refreshes highlighting on connect/reconnect and `highlights.changed`; it stops when there are no consumers or the app leaves the foreground. Each file subscription refreshes initially, including when reusing a stream. Highlight refreshes use the displayed immutable blob IDs and never switch the open revision, file, expanded context, or selection. Other live review/inbox updates belong to a later slice.

The File reader’s bottom bar explicitly marks/unmarks the selected file viewed through the hub, naming the displayed revision. It keeps the file open, including expanded context and reading position. Progress and the file list update after confirmation; a failed write retains the previous state and offers retry for that file. Only one viewed write is sent at a time; Back is temporarily hidden while saving so the inbox refresh follows the write. Trusted files count as viewed automatically and have no toggle. Viewed marks persist in the hub, so reopening the review, relaunching the app, and returning to the inbox show the saved progress. Marking viewed does not submit feedback or approve a review.

Select source text and choose **Comment** in the native selection menu to start a draft. The sheet shows the pinned item/revision, file, Before/After side, source-line range, and quoted source. Removals anchor to old lines; additions and unchanged context anchor to new lines. Selections spanning both sides or hidden source gaps cannot create a comment. Wrapped display rows never become anchors. The navigation bar’s **Comment drafts** control lists this review’s local drafts across files, with editing and confirmed deletion. Text saves synchronously through SwiftData as it changes, including unfinished edits before closing the sheet. Drafts are isolated by normalized hub address, internal item identity, and revision; they survive process termination. Storage failures retain the editor text with retry and confirmed discard recovery. Existing unchanged drafts stay readable when writing fails. These drafts stay on this iPhone until explicitly sent with **Send feedback**. Editing drafts does not change viewed state or mutate the hub.

The file’s **Conversations** control opens sent threads for that file, showing source side/lines, comment previews, reply counts, and current status. Select source text and choose **Conversations** in the native menu to filter threads to those lines; unchanged context searches both old and new source coordinates. The navigation bar opens all item threads, including whole-item and outdated comments. Each thread shows its original quote, full comment and sent replies in order, authors, times, and question/rejection/addressed/override status. External comments retain their GitHub author, delivery/vote state, and file identity even when they have no source anchor. Earlier comments use hub-provided placement onto the displayed revision; changed, outdated, and later-revision anchors are labeled explicitly. Opening a sheet fetches authoritative conversations with `onto=<displayed revision>`; pull to refresh and Retry preserve the last readable response on failure. Reading threads keeps the diff, selection, expanded context, viewed state, and local drafts intact. Sending replies and resolving decisions remain later slices.

## Verification

From the repository root:

```sh
.claude/scripts/check.sh lint ios
.claude/scripts/check.sh test ios
```

`lint ios` generates and builds; `test ios` runs Swift Testing behavior tests and XCTest UI tests. Pixel goldens use Xcode 26.2 and iOS 26.2; CI pins both, and the default destination is iPhone 17 Pro / iOS 26.2. `MG_IOS_DESTINATION` overrides the destination for other checks. Logs and xcresult bundles are under `ios/.build/`.

Arguments after `--` reach `xcodebuild`, so a focused check can use `.claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/ConnectAndReopenTests` with the fixture's `MG_IOS_TEST_HUB_URL`.

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

For the viewed-files flow, seed a fresh disposable review and trust its binary receipt:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-viewed
python3 ios/Scripts/review-fixture.py /tmp/mergegate-ios-viewed
python3 ios/Scripts/viewed-fixture.py /tmp/mergegate-ios-viewed
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_VIEWED=1 .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-viewed
```

This flow marks/unmarks through the native control, verifies the file list and inbox counts, reopens and terminates/relaunches the app, and checks the trusted-file boundary. It audits accessibility and dark mode at the largest text size and retains screenshots. Behavior tests cover confirmed progress, failed-write retry after changing files, serialized writes while navigating, revision/path encoding, and excluding trusted files from remaining work. Use this fixture independently of the start-review fixture flow, which expects all four files to require review.

For the persistent-draft flow, seed both real diff sides on a fresh smoke hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-drafts
python3 ios/Scripts/drafts-fixture.py /tmp/mergegate-ios-drafts --pack ~/.mergegate/languages/swift/0.7.1
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_DRAFTS=1 .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-drafts
```

Use the installed pack's actual version directory. The smoke hub starts without grammars; `--pack` installs Swift highlighting before submission so the demo shows highlighted source. Omit it to exercise plain-text fallback. Draft-sheet source quotes currently remain plain text.

The UI test selects removed and added source, creates drafts through the real native menu, terminates the app with an open editor, restores and edits feedback, navigates files, and verifies deletion across another relaunch. It audits the list/editor in normal and dark/accessibility appearances and retains screenshots. Swift behavior tests reopen an on-disk SwiftData store, verify hub/item/revision isolation, exact Unicode quotes and old/new line ranges, empty edits, save/delete failure retries, unchanged reads under write failure, and discard recovery. The fixture remains unreviewed on the hub.

## Module boundaries

- `App` composes feature builders.
- `HubConnection` owns URL validation, `/api/whoami`, connection presentation, and the selected URL in UserDefaults. `HubConnectionInterface` exposes only the established connection value needed by Inbox.
- `Inbox` owns `/api/inbox` and its consumed `/api/items/{id}` triage subset, project/state filtering, pending-decision interpretation, refresh policy, state, and rows.
- `Review` owns its `/api/items/{id}?since=all` subset, revision-scoped `/viewed` writes/retry, guide/file presentation, snapshot loading/retry, selected file, loading/retrying blobs and highlights from the displayed revision, and its highlighting event policy. Inbox accepts a destination builder; App composes it with Review without sharing DTOs.
- `Conversation` owns local comment drafts, their SwiftData schema and persistence, editing/list sheets, and storage recovery. `ConversationInterface` exposes review/source values and the snapshot/removal port used by submission.
- `ReviewSubmission` owns feedback preview, summary persistence, its delivery outbox, draft upload/reconciliation, review submission, and receipt recovery through the read-only operator event log. App injects Conversation’s draft adapter; features do not import one another’s implementation.
- `DiffDocument` maps unified hunks and unchanged blob gaps into source-coordinate lines; `DiffDocumentInterface` exposes only those rendering values.
- `CodeStyling` implements `CodeStylingInterface`, mapping neutral UTF-16 spans to native attributes; App injects the implementation into Review and DiffView.
- `DiffView` renders those values with UIKit/TextKit 2, native selection with source-only Copy and source-coordinate Comment actions, wrapping, and feature-local generated appearance.
- `Networking` provides transport primitives: bounded URLSession GET/JSON PUT/POST/DELETE requests, hub error decoding, rejection of redirects, and a multiplexed reconnecting SSE byte stream. It owns no endpoint or feature refresh policy.
- `Appearance/Appearance.swift` is generated from [shared tokens](../appearance/README.md) and compiled privately into features. DM Sans is registered by the app; semantic colors follow light/dark appearance and typography scales with Dynamic Type.

The Rust files `crates/api/src/device.rs`, `item.rs`, `diff.rs`, and `review.rs` remain the wire authority. Feature DTOs intentionally decode their consumed fields and ignore additional fields; the real-hub UI flow exercises the contract.

For the conversations flow, seed sent threads and replies on a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-conversations
python3 ios/Scripts/conversations-fixture.py /tmp/mergegate-ios-conversations
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_CONVERSATIONS=1 \
  .claude/scripts/check.sh test ios
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-conversations
```

The UI flow reads old/new-side threads, questions and sent operator answers, rejection reasons, file-scoped and all-item lists, and native source-selection filtering. It verifies dark mode and the largest Dynamic Type size, audits accessibility, and retains screenshots. Behavior tests cover pinned revision query encoding, transport retry, preserving the last response on failure, moved/changed/outdated/future anchors, both sides of unchanged context, accepted rejections followed by overrides, unplaced GitHub file comments, and keeping unsent drafts out of sent history. All fixture mutations use the isolated hub’s real routes and CLI; the app only reads conversations.

For the feedback flow, seed both diff sides on a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-feedback
python3 ios/Scripts/drafts-fixture.py /tmp/mergegate-ios-feedback --pack ~/.mergegate/languages/swift/0.7.1
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_FEEDBACK=1 \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/SendFeedbackTests
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-feedback
```

**Send feedback** opens a native completion sheet showing the pinned item/revision, local comments with source quotes and Before/After line ranges, and a persistent summary. Any existing hub drafts and reply text are shown too, because the hub sends them with the review. Empty local drafts stay on the phone. Sending uploads the exact source anchors, then submits the `comment` verdict; the hub owns resulting item state. It does not approve or merge. A newer revision or closed item blocks delivery and preserves local work. Changed hub/local drafts require reading the updated preview before sending.

A feature-owned SwiftData outbox freezes the confirmed content before the first upload. Partial delivery can resume after relaunch, matching uploaded comments without creating duplicates. A lost review response is reconciled against the read-only operator event log, without acknowledging workflow feedback. Unconfirmed delivery offers Check delivery; Send again requires an explicit duplicate-summary confirmation. Before review delivery starts, Edit feedback removes unchanged uploaded copies and restores an editable preview while retaining local work. Only confirmed local snapshots are removed; later edits survive. Storage or network failures retain feedback for recovery. The UI flow creates comments on both source sides, restores the summary after termination, sends, checks the real hub event’s revision/anchors/text, verifies conversations and local cleanup, and audits normal/dark/largest-text appearances. Behavior tests cover partial uploads, lost responses, summary-only delivery, shared hub drafts, stale/closed items, outbox scope, and cleanup retry.

The inbox UI audits retain all checks. For identified inbox labels that XCTest reports as low contrast, a narrow fallback verifies substantial foreground/background populations in the actual rendered label crop and requires at least 4.5:1 contrast. Other labels and audit types still fail normally. An unattributed contrast report retains its diagnostics and receives one complete audit retry without changing the UI; unresolved reports still fail. This handles the [contrast false positives documented by Apple](https://developer.apple.com/videos/play/wwdc2023/10035/) while retaining rendered-color coverage; a regression test rejects low contrast, uniform crops, gradients with low contrast, and isolated dark pixels.
