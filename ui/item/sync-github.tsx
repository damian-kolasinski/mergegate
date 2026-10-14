import { createSignal, Show } from "solid-js";
import type { Synced } from "../api/Synced";
import { postJson } from "../app/hub";
import { Button, Meta } from "../design-system";
import styles from "./sync-github.module.css";

/**
 * Reads the pull request's comments now and hands those the hub holds to
 * the agent: the hub polls on its own, so this is for when it missed some.
 * What it finds reaches the page as `item.changed`.
 */
export function SyncGithub(props: { item: string }) {
  const [syncing, setSyncing] = createSignal(false);
  const [said, setSaid] = createSignal<{ text: string; failed: boolean }>();
  const sync = () => {
    setSyncing(true);
    setSaid(undefined);
    postJson<Synced>(`/api/items/${encodeURIComponent(props.item)}/sync`, {})
      .then(
        (synced) => setSaid({ text: summary(synced), failed: false }),
        (error: unknown) =>
          setSaid({
            text: error instanceof Error ? error.message : String(error),
            failed: true,
          }),
      )
      .finally(() => setSyncing(false));
  };
  return (
    <span class={styles.sync}>
      <Show when={said()}>
        {(said) => (
          <span role={said().failed ? "alert" : "status"}>
            <Meta tone={said().failed ? "removed" : "subtle"}>
              {said().text}
            </Meta>
          </span>
        )}
      </Show>
      <Button disabled={syncing()} onClick={sync}>
        Sync GitHub
      </Button>
    </span>
  );
}

function summary(synced: Synced): string {
  const found = synced.new + synced.edited;
  const parts = found
    ? [`${synced.new} new`, `${synced.edited} edited`]
    : ["Up to date"];
  if (synced.released) {
    parts.push(`${synced.released} sent to the agent`);
  }
  return parts.join(", ");
}
