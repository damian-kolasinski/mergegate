import { createSignal, Show } from "solid-js";
import type { Health } from "../api/Health";
import { Meta } from "../design-system";
import { getJson, onHubEvent } from "./hub";

/**
 * The `mg` running the hub, so the operator can tell a hub left running
 * across an upgrade from a current one. Shows nothing until the hub answers.
 */
export function HubVersion() {
  const [version, setVersion] = createSignal<string>();
  const load = () =>
    getJson<Health>("/api/health").then(
      (health) => setVersion(health.version),
      () => undefined,
    );
  void load();
  // A reconnect may be to a restarted hub, upgraded in between; no event
  // means one. The inbox's own changes don't move the version.
  onHubEvent("inbox.changed", (event) => {
    if (!event) void load();
  });
  return (
    <Show when={version()}>
      {(current) => (
        <span title="The mergegate running the hub">
          <Meta>{current()}</Meta>
        </span>
      )}
    </Show>
  );
}
