import { createSignal, Show } from "solid-js";
import type { Synced } from "../api/Synced";
import { type Failure, failureOf, postJson } from "../app/hub";
import { Button, ErrorNote, Meta } from "../design-system";
import styles from "./sync-github.module.css";

/**
 * Reads the pull request's comments now and hands those the hub holds to
 * the agent: the hub polls on its own, so this is for when it missed some.
 * What it finds reaches the page as `item.changed`.
 */
export function SyncGithub(props: { item: string }) {
  const [syncing, setSyncing] = createSignal(false);
  const [said, setSaid] = createSignal<string>();
  const [failure, setFailure] = createSignal<Failure>();
  const sync = () => {
    setSyncing(true);
    setSaid(undefined);
    setFailure(undefined);
    postJson<Synced>(`/api/items/${encodeURIComponent(props.item)}/sync`, {})
      .then(
        (synced) => setSaid(summary(synced)),
        (error: unknown) => setFailure(failureOf(error)),
      )
      .finally(() => setSyncing(false));
  };
  return (
    <span class={styles.sync}>
      <Button busy={syncing()} onClick={sync}>
        {syncing() ? "Fetching…" : "Fetch GitHub comments"}
      </Button>
      <Show when={said()}>
        {(said) => (
          <span role="status">
            <Meta>{said()}</Meta>
          </span>
        )}
      </Show>
      <Show when={failure()}>
        {(failure) => <ErrorNote failure={failure()} />}
      </Show>
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
