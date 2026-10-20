import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import type { Health } from "../api/Health";
import { Meta } from "../design-system";
import { getJson, onHubEvent } from "./hub";

// The version the hub first answered with, which is the one whose UI this
// page runs: the hub served it. It belongs to the loaded bundle, not to the
// component.
let served: string | undefined;
let reloading = false;

// What screens hold that a reload would lose. A signal, so a reload an
// upgrade put off goes ahead as soon as the last of them lets go.
const [holds, setHolds] = createSignal<readonly (() => boolean)[]>([]);

/**
 * Keeps an upgraded hub from reloading the page while `holding` is true, for
 * as long as the calling component lives.
 */
export function useReloadHold(holding: () => boolean): void {
  setHolds((all) => [...all, holding]);
  onCleanup(() => setHolds((all) => all.filter((other) => other !== holding)));
}

/**
 * Whether the page is unloading because the hub was upgraded, which only
 * happens once nothing holds the reload: a screen asking the browser to
 * confirm the unload has no need to.
 */
export function reloadingOntoUpgrade(): boolean {
  return reloading;
}

/**
 * The `mg` running the hub, so the operator can tell a hub left running
 * across an upgrade from a current one. Shows nothing until the hub answers.
 * A page still on an older hub's UI reloads onto the new one, once nothing
 * on it holds the reload.
 */
export function HubVersion() {
  const [version, setVersion] = createSignal<string>();
  const load = () =>
    getJson<Health>("/api/health").then(
      (health) => {
        served ??= health.version;
        setVersion(health.version);
      },
      () => undefined,
    );
  void load();
  // A reconnect may be to a restarted hub, upgraded in between; no event
  // means one. The inbox's own changes don't move the version.
  onHubEvent("inbox.changed", (event) => {
    if (!event) void load();
  });
  const upgraded = () => version() !== undefined && version() !== served;
  createEffect(() => {
    if (upgraded() && !holds().some((holding) => holding())) {
      reloading = true;
      location.reload();
    }
  });
  return (
    <Show when={version()}>
      {(current) => (
        <span
          title={
            upgraded()
              ? "The hub was upgraded: the page reloads onto its UI once nothing unsaved is open"
              : "The mergegate running the hub"
          }
        >
          <Meta>
            {upgraded() ? `${current()} · reload pending` : current()}
          </Meta>
        </span>
      )}
    </Show>
  );
}
