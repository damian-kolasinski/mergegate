import { createSignal, For, Match, Show, Switch } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { Merge } from "../api/Merge";
import { deleteJson, postJson } from "../app/hub";
import { Badge, Button, Meta, Mono } from "../design-system";
import styles from "./merge-blocked.module.css";

/**
 * Why the newest revision's merge didn't land. A held merge waits for the
 * operator to retry or cancel it; a conflict is already with the agent, who
 * was told how to rebase.
 */
export function MergeBlocked(props: {
  item: string;
  target: string;
  blocked: Blocked;
  /** The hub moved the item: a retry landed or held again, or a cancel. */
  onChanged: () => void;
}) {
  const [sending, setSending] = createSignal(false);
  const [failure, setFailure] = createSignal<string>();
  const path = () => `/api/items/${encodeURIComponent(props.item)}/merge`;
  const send = (request: Promise<unknown>) => {
    setSending(true);
    setFailure(undefined);
    request
      .then(props.onChanged, (error: unknown) =>
        setFailure(error instanceof Error ? error.message : String(error)),
      )
      .finally(() => setSending(false));
  };
  const cancel = () => send(deleteJson(path()));
  const retry = () => send(postJson<Merge>(path(), {}));

  return (
    <Switch>
      <Match when={props.blocked.kind === "held" && props.blocked}>
        {(held) => (
          <section class={styles.notice} aria-label="Merge held">
            <Badge tone="warning">Merge held</Badge>
            <Meta>{held().reason}</Meta>
            <span class={styles.actions}>
              <Show when={failure()}>
                {(message) => <Meta tone="removed">{message()}</Meta>}
              </Show>
              <Button disabled={sending()} onClick={cancel}>
                Cancel merge
              </Button>
              <Button variant="solid" disabled={sending()} onClick={retry}>
                Retry merge
              </Button>
            </span>
          </section>
        )}
      </Match>
      <Match when={props.blocked.kind === "conflict" && props.blocked}>
        {(conflict) => (
          <section class={styles.notice} aria-label="Merge conflict">
            <Badge tone="warning">Conflict</Badge>
            <Meta>
              Conflicts with <Mono>{props.target}</Mono> at{" "}
              <Mono>{conflict().target_commit.slice(0, 12)}</Mono> in
            </Meta>
            <ul class={styles.paths}>
              <For each={conflict().paths}>
                {(file) => (
                  <li>
                    <Mono>{file}</Mono>
                  </li>
                )}
              </For>
            </ul>
            <Meta>Sent back to the agent to rebase.</Meta>
          </section>
        )}
      </Match>
    </Switch>
  );
}
