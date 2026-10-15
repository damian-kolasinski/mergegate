# 19 — Multi-machine

Roadmap row 19 · decisions settled 2026-10-15

## Scope

Agents on other machines of the operator's tailnet submit to the hub and get their feedback, as on the hub's machine: F2 end to end (listen on the tailnet, `mg connect`, Tailscale `whois`, `mg hub allow`, `mg init`, `mg doctor` on the worker). Remote workers can't set or keep a wake (design.md §4 "Multi-machine & security", §6.5). The full `mg doctor` and installing `mg` on a worker are 18a.

## Decisions

1. **Tailscale only.** Tokens leave the design: no `mg hub token create`, no `mg connect --token`, no listening beyond loopback and the tailnet. Every machine runs on the operator's tailnet.
2. **A trusted device reaches everything** a loopback caller does, the UI included, so the operator can review from a laptop. The wake rules below are the only difference.
3. **The `tailscale` CLI, not the LocalAPI** (closes tech-stack §11): `tailscale status --json` and `tailscale whois --json`, the same on the macOS app and Linux, about 25 ms a call, each peer's answer cached.
4. *(agent)* **Remote is decided by the listener** a connection arrived on, never by what the client sends. The tailnet listener binds the hub's Tailscale IPv4 on the hub's port; loopback stays as it is.
5. *(agent)* **`mg hub set listen local|tailnet`** applies at once and answers with the tailnet URL, as `http://<magicdns short name>:<port>`. If Tailscale isn't up (a hub started at login before it), the hub keeps retrying and `mg doctor` says why. The Host guard on that listener accepts the hub's short name, full MagicDNS name and IP, so DNS rebinding stays shut.
6. *(agent)* **Who's trusted:** a device owned by the hub's Tailscale user, unless denied; any other device once `mg hub allow`ed. Tagged devices have no owner, so they need `allow`. `allow` and `deny` resolve a name through `tailscale status` and keep the device's StableID, so a rename keeps its access; `deny` overrides owner trust, `allow` undoes a `deny`.
7. *(agent)* **An unknown device is refused** with `device_not_allowed`, naming it and its owner, hinting `mg hub allow <device>` on the hub, and listed as "asked" in `mg hub devices`. F2's `needs_decision` doesn't fit: the worker can't re-run with a flag that allows itself.
8. *(agent)* **`mg connect <host[:port]|url>`** checks the hub's health and how it sees this device (`GET /api/whoami`), then writes `connect.json` in `MERGEGATE_HOME`, which wins over `hub.json`. It writes nothing when refused. `mg connect --local` goes back to the local hub. `mg hub start|stop|status` always mean the local hub; status names the connection.
9. *(agent)* **Wake:** a remote submit with `--wake` (even `""`) is refused with `wake_remote` before anything is recorded; `mg` refuses it before pushing when connected. A remote submit clears the item's wake.
10. *(agent)* **Tests fake Tailscale** with a `tailscale` script on `PATH` (as `gh` is faked) reading JSON the test writes. `MG_TAILNET_ADDR` binds the tailnet listener on another loopback port, and `MG_TAILSCALE_CACHE_MS` / `MG_TAILNET_RETRY_MS` shorten the cache and retry.

## Where it lives

- `core`: `device` (identity, access, the trust rule, `DeviceStore` and the `Tailnet` port), the `listen` setting.
- New adapter crate `tailscale` (tech-stack's `auth-tailscale` and `auth-token` become this one).
- `store-sqlite`: `0011_devices.sql`, `devices.rs`.
- `api::device`, `api::settings` gains `listen`; `hub`: `tailnet` (listener and guard), `devices` routes, `whoami`; `guard`, `items` (wake), `doctor` and `settings` touched.
- `cli`: `connect_cmd`, `hub_cmd/devices`, `client` and `probe` read `connect.json`; skill reference for setup.
- `testkit`: `FakeTailscale`.

## Stack

- `19-multi-machine`: everything above, F2 scenario tests, doc updates, deleting this brief and roadmap row 19. *In progress.*

## Open

- **Doc drift:** design.md §4, §6.0 and §12 (decision 8) still name tokens; user-flows F2's last note and its `needs_decision` step; tech-stack §4's crate list and slice table, §11's spike question.
