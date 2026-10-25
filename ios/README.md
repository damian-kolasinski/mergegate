# mergegate for iOS

The review workflow described in [iOS Mobile](../iOS%20Mobile.md): connect through the installed Tailscale app, reopen the selected hub, triage its Decision desk across projects and item states, start a review with a real unified diff, read surrounding source context, render hub-provided syntax highlighting, track viewed files, keep source-anchored comment drafts, follow existing conversations, send feedback with a summary, request changes, re-review revised items, resolve pending decisions, approve and merge an exact revision, and stay current through foreground live updates. iOS 26+, Swift 6, SwiftUI/Observation, and Tuist 4.206.0. There are no package dependencies.

## Build and run

Install Xcode 26.2+ and Tuist (the version is pinned in `mise.toml`). With mise, run `mise install` from this directory, then `tuist generate --no-open`. Open `Mergegate.xcworkspace`, select the Mergegate scheme and an iPhone/iPad simulator, and run. For a physical device, select your signing team on the app target in Xcode. Generated projects and build outputs are ignored.

On the hub machine run `mg hub set listen tailnet`. Enable the installed Tailscale app on the phone, then enter the URL the command prints. Short MagicDNS names, full `.ts.net` names, and Tailscale IPv4/IPv6 addresses are accepted. The hub must admit the caller through `/api/whoami`; the app cannot allow itself. Denied/unknown devices receive the hub's error and recovery hint. Only a successful connection is saved. Hub → Forget hub clears it.

The inbox refreshes on entry, after returning from a review, on pull to refresh/retry, and when foregrounded while visible. A failed refresh leaves the last response visible with an error. Reopening a saved hub revalidates device trust before displaying its inbox. The native large-title navigation bar collapses as the inbox scrolls. Its project menu, All/Decide/Review chips, floating state bar, and detail controls use iOS 26 Liquid Glass; review content stays on solid surfaces. Native materials follow the system’s Reduce Transparency and Reduce Motion settings. The project menu and action chips keep decisions/questions ahead of reviewable work; other states are available in the footer. Decision cards show the pending question or disputed comment, with the full text and decision actions in a native sheet; working, merging, and closed-today items follow. Visible open rows load the real pending question, disputed comment and rejection reason, or comment awaiting a vote, plus unviewed review-file counts (trusted files are excluded). Each detail response must match the inbox revision; a mismatch asks for an inbox refresh. Per-item failures have their own retry. Persistent offline caching belongs to the final slice.

Every inbox item opens its current authoritative review. The screen shows the item identity, revision and target, all-changes comparison scope, and remaining review-file count. Guide opens the complete submitted guide; Files preserves the hub’s importance order and shows its reading reasons, viewed state, and trusted tier. The first review-tier file opens automatically (or the first trusted file when there are no review-tier files). The native TextKit 2 reader displays unified hunks with a compact line-number gutter (old coordinates for removals, new coordinates otherwise), explicit addition/removal markers, and shared semantic colors. Wrapped continuations align with the source column; all blocks in a file share its gutter width. Hunk headers retain both source ranges. Binary, oversized, discarded, and metadata-only diffs have explicit states. Opening or reading never marks a file viewed. The displayed revision stays fixed while open, with a notice when a newer one arrives, including across foregrounding and comparison changes; returning to the inbox and reopening loads the current revision. The entire review scrolls at accessibility text sizes.

Load unchanged context fetches the comparison’s immutable old/new blobs, then exposes leading, between-hunk, and trailing gaps in batches of 20 lines. A failed expansion leaves the diff readable and can be retried. Renamed and metadata-only text files can expose their unchanged source too. Long lines wrap at the reader width; native selection and Copy preserve source text and Unicode while excluding gutters, change markers, hunk headers, and no-newline notes. Selection spans one displayed text block; expanding gaps creates separate blocks.

The reader applies hub-provided UTF-16 syntax spans in source coordinates before wrapping, using the same generated light/dark palette as the web. Removed lines use old-blob spans; additions and context use new-blob spans. The text, diff backgrounds, gutters, and source-only Copy remain intact. Missing grammars, pending spans, failed highlighting requests, unknown tags, and invalid ranges leave readable plain source. One shared foreground SSE transport refreshes highlighting on connect/reconnect and `highlights.changed`; it stops when there are no consumers or the app leaves the foreground. Each file subscription refreshes initially, including when reusing a stream. Highlight refreshes use the displayed immutable blob IDs and never switch the open revision, file, expanded context, or selection. Inbox, review, and conversation updates share this same transport.

The File reader’s bottom bar explicitly marks/unmarks the selected file viewed through the hub, naming the displayed revision. It keeps the file open, including expanded context and reading position. Progress and the file list update after confirmation; a failed write retains the previous state and offers retry for that file. Only one viewed write is sent at a time; Back is temporarily hidden while saving so the inbox refresh follows the write. Trusted files count as viewed automatically and have no toggle. Viewed marks persist in the hub, so reopening the review, relaunching the app, and returning to the inbox show the saved progress. Marking viewed does not submit feedback or approve a review.

Select source text and choose **Comment** in the native selection menu to start a draft. The sheet shows the pinned item/revision, file, Before/After side, source-line range, and quoted source. Removals anchor to old lines; additions and unchanged context anchor to new lines. Selections spanning both sides or hidden source gaps cannot create a comment. Wrapped display rows never become anchors. The navigation bar’s **Comment drafts** control lists this review’s local drafts across files, with editing and confirmed deletion. Text saves synchronously through SwiftData as it changes, including unfinished edits before closing the sheet. Drafts are isolated by normalized hub address, internal item identity, and revision; they survive process termination. Storage failures retain the editor text with retry and confirmed discard recovery. Existing unchanged drafts stay readable when writing fails. These drafts stay on this iPhone until explicitly sent with **Send feedback**. Editing drafts does not change viewed state or mutate the hub.

The file’s **Conversations** control opens sent threads for that file, showing source side/lines, comment previews, reply counts, and current status. Select source text and choose **Conversations** in the native menu to filter threads to those lines; unchanged context searches both old and new source coordinates. The navigation bar opens all item threads, including whole-item and outdated comments. Each thread shows its original quote, full comment and sent replies in order, authors, times, and question/rejection/addressed/override status. External comments retain their GitHub author, delivery/vote state, and file identity even when they have no source anchor. Earlier comments use hub-provided placement onto the displayed revision; changed, outdated, and later-revision anchors are labeled explicitly. Opening a sheet fetches authoritative conversations with `onto=<displayed revision>`; pull to refresh and Retry preserve the last readable response on failure. Reading threads keeps the diff, selection, expanded context, viewed state, and local drafts intact. Pending question answers and external rejection decisions are available from the Decision desk.

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
- `Inbox` owns `/api/inbox` and its consumed `/api/items/{id}` triage subset, project/state filtering, pending-decision interpretation, question-answer drafts, rejection decisions, refresh policy, state, and rows. App injects ReviewSubmission’s feedback sheet through the ConversationInterface scope.
- `Review` owns its `/api/items/{id}?since=all|<revision>` subset, revision-scoped `/viewed` writes/retry, guide/file presentation, snapshot loading/retry, selected file, loading/retrying blobs and highlights from the displayed revision, and its highlighting event policy. Inbox accepts a destination builder; App composes it with Review without sharing DTOs.
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

For the request-changes flow, seed a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-request-changes
python3 ios/Scripts/drafts-fixture.py /tmp/mergegate-ios-request-changes --pack ~/.mergegate/languages/swift/0.7.1
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_REQUEST_CHANGES=1 \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/RequestChangesTests
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-request-changes
```

The completion sheet’s **Review outcome** menu selects **Send feedback** or **Request changes**. Request changes requires a summary or comments/replies describing the work, sends the pinned revision through the existing review endpoint, and returns the item to **In progress** in the Decision desk. The hub remains authoritative for the transition. The selected outcome persists with the summary and is frozen with each delivery attempt, including partial uploads, relaunch, lost-response reconciliation, and editing retained feedback. Existing outbox records default to Send feedback. The receipt must match the frozen verdict before local feedback is cleared.

The UI flow sends an actionable summary, restores the selected outcome after termination, verifies the exact verdict/revision/text in the real hub event log, and checks the item moves out of Needs you into In progress. It audits normal/light and largest-text/dark sheets and retains screenshots. Behavior tests cover comment-bearing requests, summary-only requests, empty/stale prevention, outcome persistence, retries, and avoiding confirmation from another verdict’s receipt.

For re-review, seed two revisions through the CLI on a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-rereview
python3 ios/Scripts/rereview-fixture.py /tmp/mergegate-ios-rereview
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_REREVIEW=1 \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/ReReviewRevisionTests
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-rereview
```

Reopening a revised item defaults to changes since the last reviewed revision when the hub offers that comparison. The compact comparison menu also offers all changes against the base and earlier available revisions. Switching retains the selected file when present, resets source context/selection to the new immutable blobs, and keeps the same displayed revision. A failed request leaves the readable comparison intact; choose again to retry. If the hub has a newer revision, return to the inbox and reopen rather than silently replacing active review work. Local drafts remain scoped to their original revision.

The hub carries viewed marks only for unchanged content and maps earlier comments onto the displayed revision. File labels distinguish changed, unchanged, added, deleted, dropped, and unseparated changes. Dropped changes remain readable but have no viewed mark or source actions in the newest revision. An interdiff’s Before coordinates belong to a replay of the earlier revision, so native comment/thread actions use only its After coordinates; unchanged/unseparated files show the full diff and allow both sides. All changes exposes the base side. Copy works on every side; all-item/file conversations retain moved, changed, and outdated history.

The fixture asserts real viewed-state carryover and seeds moved/changed/outdated comments. The UI flow checks the last-reviewed comparison, base switching, carried viewed progress and source placement, relaunch, and normal/dark/largest-text accessibility, retaining demo screenshots. Behavior tests cover retry, a newer revision arriving during a comparison request, identity/comparison validation, dropped files, and native menu source-side boundaries.

## Resolve pending decisions

Decision desk → Read details → **Answer question** or **Resolve rejection** keeps the item, revision, original comment, and latest response together. Question answers save as hub draft replies, survive relaunch, and remain pending until **Review & send feedback** sends them. The existing feedback preview includes every local/hub comment and reply it will send; delivery and lost-response recovery verify reply IDs, parent comments, and text too. Closing feedback reconciles the answer’s sent status. Overrides send the comment and optional note immediately; accepting a rejection confirms agreement without sending feedback. Returning to the inbox refreshes its pending counts.

Before writing, the client refetches the displayed revision and latest reply, retains editor text on failure, and rejects stale, closed, or already changed decisions. Retrying a lost answer-save response updates the saved reply; retrying an uncertain override/accept first checks the authoritative result. Decisions use existing hub routes and review rules. No schema migration or older-client compatibility layer is added.

For the end-to-end decision flow, run the web suite once to build testkit’s `fake-github`, then keep this fixture process running in a separate terminal:

```sh
python3 ios/Scripts/decisions-fixture.py /tmp/mergegate-ios-decisions \
  --fake-github target/debug/fake-github
# Use the URL printed by the fixture:
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_DECISIONS=1 \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/ResolveDecisionsTests
# Ctrl-C stops both disposable services.
```

Use a fresh directory. This fixture uses the installed `mg`, an isolated home and git repository, real Tailscale trust, and testkit’s local GitHub server to ingest external comments without contacting GitHub. All native requests, review decisions, and feedback delivery go through the real hub. The UI flow saves/reopens/sends an answer, verifies its event and cleared question count, overrides one rejection, accepts another, and verifies the item leaves pending decisions. It retains simulator screenshots and audits normal appearance and dark mode at the largest text size. Behavior tests cover changed decisions/revisions, closed items, retry, exact receipts, and lost-response reconciliation.

## Approve and merge

File reader → **Complete review** → **Review outcome** → **Approve & merge** previews the same local/hub comments and replies as feedback. The summary is optional. A native confirmation names the item, exact revision, and target branch before approval sends the previewed feedback and asks the hub to squash-merge. Revision, target, item state, and draft contents are checked again before delivery; a stale confirmation preserves local work and asks you to reopen the current revision.

The result distinguishes **Merged** (with the target commit), **Merge held** (with its reason), **Rebase needed** (with conflicting paths), and **Waiting for parent**. Returning to the Decision desk refreshes its authoritative state, including Closed today. Approval uses the existing feature-owned delivery outbox: a lost response checks the operator event log before any retry, recovers merged/held/conflicting results, and clears local feedback only after the exact approval receipt is confirmed. An unresolved delivery remains explicit; another attempt requires revision/target confirmation again.

Run the end-to-end flow on a fresh disposable repository:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-approval
python3 ios/Scripts/drafts-fixture.py /tmp/mergegate-ios-approval
python3 ios/Scripts/approval-fixture.py /tmp/mergegate-ios-approval
# Keep the controller running; use its printed URL and the smoke hub's tailnet URL:
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_APPROVAL=1 \
  MG_IOS_TEST_CONTROL_URL=http://127.0.0.1:CONTROL_PORT \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/ApproveAndMergeTests
# Ctrl-C stops the controller, then:
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-approval
```

The controller only advances this fixture's real CLI revision and verifies the local target tree/commit. The native flow opens confirmation for r1, submits r2 through the controller, verifies stale approval is rejected, reopens r2, checks light/dark/largest-text accessibility and relaunch, confirms the exact revision/target, and verifies the resulting git target and Closed today inbox. Screenshots remain in the Xcode result bundle. Behavior tests cover approval with both source-side comments, hub drafts/replies, empty summaries, exact target confirmation, endpoint freshness rejection, closed items, uncertain retries/relaunch, and recovery of merged/held/conflicting results.

## Stay current while open

The foreground Decision desk, File reader, and sent conversations share one SSE connection per hub. Features subscribe before their initial read, interpret their own `inbox.changed`/`item.changed` invalidations, and refetch on reconnect or foregrounding. Submitted reviews invalidate conversations too. Leaving the foreground cancels consumers; the transport closes when its last consumer leaves. Failed reads retain their last response with retry; overlapping reads cannot overwrite a newer response.

The File reader keeps its displayed revision, file, comparison, expanded context, source selection, and local draft scope. Same-revision refreshes reconcile viewed progress; a newer revision produces a compact notice explaining how to open it from the inbox. Active draft sheets stay open with their existing text and revision. Disconnect notices keep retained content visible while the stream retries; reconnect checks the authoritative revision even when events were missed. No persistence schema or compatibility layer is added.

Verify live behavior against a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-live
python3 ios/Scripts/live-updates-fixture.py /tmp/mergegate-ios-live \
  --pack ~/.mergegate/languages/swift/0.7.1
# Keep the controller running; use its printed URL and the hub's tailnet URL:
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_LIVE_UPDATES=1 \
  MG_IOS_TEST_CONTROL_URL=http://127.0.0.1:CONTROL_PORT \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/StayCurrentTests
# Ctrl-C stops the controller, then:
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-live
```

The optional `--pack` installs an existing Swift grammar; omit it to use plain code. The controller submits real CLI items/revisions/replies and restarts only its isolated hub. The native flow checks live inbox arrivals and replies in an already-open thread, edits a pinned draft across revision changes, stops/restarts the hub, verifies missed-revision recovery after backgrounding, and explicitly reopens the latest revision. It retains demo screenshots and audits light and dark/largest-text appearances. Behavior tests cover event filtering, submitted-review invalidation, failure/retry, cancelled/overlapping refreshes, revision pinning, and a stale read racing viewed progress.

## Resume offline work

Previously opened reviews retain their guide, ordered files, diff hunks and highlighting on this iPhone. Loaded source blobs and their syntax spans remain available for unchanged context; unopened source still requires the hub. Reopening restores the selected comparison/file, expanded context and scroll position for that revision. Saved inbox/review notices show when the snapshot was stored. A missing or unreadable cache produces the normal retry screen.

The saved trusted hub opens without requiring a new network handshake. This only permits local reading: every remote operation still uses the hub's existing authorization. Inbox and Review own their separate caches; Review owns SwiftData reading metadata and files for source/snapshots. Comment drafts and summaries continue using their existing feature-owned durable stores. No migration or legacy support is added.

On reconnect, the reader checks the authoritative revision and progress while keeping the displayed code and draft scope. **Open rN** explicitly opens a newer revision; earlier drafts remain on their original revision. The completion sheet preserves offline edits and offers **Reconnect & check revision**. Sending requires a successful current-revision check and revalidates immediately before delivery. Stale or closed revisions retain feedback and cannot send.

Verify against a fresh disposable hub with an installed Swift grammar:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-offline
python3 ios/Scripts/offline-fixture.py /tmp/mergegate-ios-offline \
  --pack ~/.mergegate/languages/swift/0.7.1
# Keep the controller running, using its printed URL and the hub's tailnet URL:
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_OFFLINE=1 \
  MG_IOS_TEST_CONTROL_URL=http://127.0.0.1:CONTROL_PORT \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/ResumeOfflineTests
# Ctrl-C stops the controller, then:
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-offline
```

The native flow caches real source/highlighting, stops the isolated hub, terminates/reopens the app, restores expanded context and edits local feedback offline. It then restarts the hub with a CLI-submitted revision, verifies stale feedback cannot send, explicitly opens the current revision and verifies its delivered summary in the real hub event log. It retains demo screenshots and audits light and dark/largest-text appearances. Behavior tests cover comparison/position restoration, immutable source and span reuse, hub/item isolation, corrupt cache, authorization failures, reconnect revision pinning, delayed refreshes, and same-revision versus stale feedback recovery.


## Web feature parity

The native client exposes the web operator workflow through native navigation and sheets:

| Area | Native controls |
| --- | --- |
| Inbox | Project/state/action filters, pending decisions and rule changes, priority/notes, stack/merge flags, revision changes, demos and weekly closures |
| Review | Importance order and full ranking reasons, viewed state, any available interdiff, source context, highlighting, source selection/copy and conversation-to-source navigation |
| Content | Rendered guide and Markdown previews, before/after images, overlay and exact pixel differences (including resized images), original-file sharing, screenshot/video demos and current demo ZIP sharing |
| Conversations | Sent history, GitHub links, local comments, hub draft editing/deletion/re-placement, reply creation/editing/deletion, votes including lapsed/dismissed comments, acceptance and override of rejections |
| Completion | Feedback, changes requested, exact-revision approval, merge outcome, next review, held-merge retry/cancellation, confirmed abandonment, GitHub comment sync and priority editing |
| Rules | Applied rules, proposal previews and affected files, approve/reject proposals and held edits, explicit confirmation before overwriting the rules file |

The native reader uses a single column and touch/source-selection controls in place of desktop keyboard navigation. It also retains offline reviews, drafts and reading position. Image decoding is bounded to 64 MB and 16 million pixels; an image pair whose combined extent exceeds the comparison limit remains readable side by side. Markdown and guide rendering never load remote images or run HTML. Active review time includes guide/thread scrolling, typing and bounded video playback, and is settled before feedback delivery or abandonment.

Run the new parity flow with a fresh disposable hub:

```sh
python3 ios/Scripts/smoke-hub.py start /tmp/mergegate-ios-parity
python3 ios/Scripts/parity-fixture.py /tmp/mergegate-ios-parity
MG_IOS_TEST_HUB_URL=http://my-mac:PORT MG_IOS_TEST_PARITY=1 \
  .claude/scripts/check.sh test ios -- -only-testing:MergegateUITests/WebParityTests
python3 ios/Scripts/parity-fixture.py /tmp/mergegate-ios-parity --verify
python3 ios/Scripts/smoke-hub.py stop /tmp/mergegate-ios-parity
```

This fixture creates only disposable local repositories and an isolated hub. The flow approves a proposed rule, changes priority, reads attached media, previews Markdown and images, saves a thread reply, jumps to source, submits real feedback, abandons the item and opens the next review. Screenshots remain in the Xcode result bundle. Behavior tests cover stale decisions, mutable ZIP downloads, resized pixel differences, same-revision rule reclassification, exact-line revealing, idle review time, and repeated lost reply responses with intervening edits.
