# mergegate for iOS

The first two slices of [iOS Mobile](../iOS%20Mobile.md): connect through the installed Tailscale app, reopen the selected hub, and triage its Decision desk across projects and item states. iOS 26+, Swift 6, SwiftUI/Observation, and Tuist 4.206.0. There are no package dependencies.

## Build and run

Install Xcode 26.2+ and Tuist (the version is pinned in `mise.toml`). With mise, run `mise install` from this directory, then `tuist generate --no-open`. Open `Mergegate.xcworkspace`, select the Mergegate scheme and an iPhone/iPad simulator, and run. For a physical device, select your signing team on the app target in Xcode. Generated projects and build outputs are ignored.

On the hub machine run `mg hub set listen tailnet`. Enable the installed Tailscale app on the phone, then enter the URL the command prints. Short MagicDNS names, full `.ts.net` names, and Tailscale IPv4/IPv6 addresses are accepted. The hub must admit the caller through `/api/whoami`; the app cannot allow itself. Denied/unknown devices receive the hub's error and recovery hint. Only a successful connection is saved. Hub → Forget hub clears it.

The inbox refreshes on entry, pull to refresh, retry, and returning to the foreground. A failed refresh leaves the last response visible with an error. Reopening a saved hub revalidates device trust before displaying its inbox. The native large-title navigation bar collapses as the inbox scrolls. Its project menu, All/Decide/Review chips, floating state bar, and detail controls use iOS 26 Liquid Glass; review content stays on solid surfaces. Native materials follow the system’s Reduce Transparency and Reduce Motion settings. The project menu and action chips keep decisions/questions ahead of reviewable work; other states are available in the footer. Decision cards show the pending question or disputed comment, with the full text in a read-only native sheet; working, merging, and closed-today items follow. Visible open rows load the real pending question, disputed comment and rejection reason, or comment awaiting a vote, plus unviewed review-file counts (trusted files are excluded). Each detail response must match the inbox revision; a mismatch asks for an inbox refresh. Per-item failures have their own retry. Persistent offline caching, SSE, opening a review, and decision/review actions belong to later slices.

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

## Module boundaries

- `App` composes feature builders.
- `HubConnection` owns URL validation, `/api/whoami`, connection presentation, and the selected URL in UserDefaults. `HubConnectionInterface` exposes only the established connection value needed by Inbox.
- `Inbox` owns `/api/inbox` and its consumed `/api/items/{id}` triage subset, project/state filtering, pending-decision interpretation, refresh policy, state, and rows.
- `Networking` provides HTTP transport only: bounded URLSession requests, hub error decoding, and rejection of redirects. It owns no endpoint or feature refresh policy.
- `Appearance/Appearance.swift` is generated from [shared tokens](../appearance/README.md) and compiled privately into features. DM Sans is registered by the app; semantic colors follow light/dark appearance and typography scales with Dynamic Type.

The Rust files `crates/api/src/device.rs`, `item.rs`, `diff.rs`, and `review.rs` remain the wire authority. Feature DTOs intentionally decode their consumed fields and ignore additional fields; the real-hub UI flow exercises the contract.
