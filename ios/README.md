# mergegate for iOS

The first slice of [iOS Mobile](../iOS%20Mobile.md): connect through the installed Tailscale app, show the hub's real inbox, and reopen the selected hub after terminating the app. iOS 26+, Swift 6, SwiftUI/Observation, and Tuist 4.206.0. There are no package dependencies.

## Build and run

Install Xcode 26.2+ and Tuist (the version is pinned in `mise.toml`). With mise, run `mise install` from this directory, then `tuist generate --no-open`. Open `Mergegate.xcworkspace`, select the Mergegate scheme and an iPhone/iPad simulator, and run. For a physical device, select your signing team on the app target in Xcode. Generated projects and build outputs are ignored.

On the hub machine run `mg hub set listen tailnet`. Enable the installed Tailscale app on the phone, then enter the URL the command prints. Short MagicDNS names, full `.ts.net` names, and Tailscale IPv4/IPv6 addresses are accepted. The hub must admit the caller through `/api/whoami`; the app cannot allow itself. Denied/unknown devices receive the hub's error and recovery hint. Only a successful connection is saved. Hub → Forget hub clears it.

The inbox refreshes on entry, pull to refresh, retry, and returning to the foreground. A failed refresh leaves the last response visible with an error. Reopening a saved hub revalidates device trust before displaying its inbox. Persistent offline inbox caching, SSE, item navigation, review actions, and detailed triage belong to later slices.

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

## Module boundaries

- `App` composes feature builders.
- `HubConnection` owns URL validation, `/api/whoami`, connection presentation, and the selected URL in UserDefaults. `HubConnectionInterface` exposes only the established connection value needed by Inbox.
- `Inbox` owns `/api/inbox`, its consumed wire DTOs, refresh policy, state, and rows.
- `Networking` provides HTTP transport only: bounded URLSession requests, hub error decoding, and rejection of redirects. It owns no endpoint or feature refresh policy.
- `Appearance/Appearance.swift` is generated from [shared tokens](../appearance/README.md) and compiled privately into features. DM Sans is registered by the app; semantic colors follow light/dark appearance and typography scales with Dynamic Type.

The Rust files `crates/api/src/device.rs` and `crates/api/src/item.rs` remain the wire authority. Feature DTOs intentionally decode their consumed fields and ignore additional fields; the real-hub UI flow exercises the contract.
