import { createSignal, For, Match, Show, Switch } from "solid-js";
import type { Blocked } from "../api/Blocked";
import type { Merge } from "../api/Merge";
import { deleteJson, type Failure, failureOf, postJson } from "../app/hub";
import { Badge, Button, ErrorNote, Meta, Mono } from "../design-system";
import styles from "./merge-blocked.module.css";

/**
 * Why the newest revision's merge didn't land. A held merge waits for the
 * operator to retry or cancel it; a conflict is already with the agent, who
 * was told how to rebase. A stacked item's approval waits for its parent to
 * land, or, once that one has, for its agent to rebase onto the target.
 */
export function MergeBlocked(props: {
  item: string;
  target: string;
  blocked: Blocked;
  /** The hub moved the item: a retry landed or held again, or a cancel. */
  onChanged: () => void;
}) {
  const [sending, setSending] = createSignal<"cancel" | "retry">();
  const [failure, setFailure] = createSignal<Failure>();
  const path = () => `/api/items/${encodeURIComponent(props.item)}/merge`;
  const send = (kind: "cancel" | "retry") => {
    setSending(kind);
    setFailure(undefined);
    const request =
      kind === "cancel" ? deleteJson(path()) : postJson<Merge>(path(), {});
    request
      .then(props.onChanged, (error: unknown) => setFailure(failureOf(error)))
      .finally(() => setSending(undefined));
  };
  const cancel = () => send("cancel");
  const retry = () => send("retry");

  return (
    <Switch>
      <Match when={props.blocked.kind === "held" && props.blocked}>
        {(held) => {
          // The hub says what happened first, then what git said, which is
          // there for whoever wants it.
          const reason = () => split(held().reason);
          return (
            <section class={styles.notice} aria-label="Merge held">
              <Badge tone="warning" solid>
                Merge held
              </Badge>
              <span class={styles.reason}>
                <Meta>{reason().said}</Meta>
                <Show when={reason().detail}>
                  {(detail) => (
                    <details class={styles.details}>
                      <summary>Details</summary>
                      <pre>{detail()}</pre>
                    </details>
                  )}
                </Show>
              </span>
              <span class={styles.actions}>
                <Show when={failure()}>
                  {(failure) => <ErrorNote failure={failure()} />}
                </Show>
                <Button
                  variant="danger"
                  disabled={sending() !== undefined}
                  busy={sending() === "cancel"}
                  onClick={cancel}
                >
                  {sending() === "cancel" ? "Cancelling…" : "Cancel merge"}
                </Button>
                <Button
                  variant="solid"
                  disabled={sending() !== undefined}
                  busy={sending() === "retry"}
                  onClick={retry}
                >
                  {sending() === "retry" ? "Retrying…" : "Retry merge"}
                </Button>
              </span>
            </section>
          );
        }}
      </Match>
      <Match when={props.blocked.kind === "waiting" && props.blocked}>
        {(waiting) => (
          <section class={styles.notice} aria-label="Merge waiting">
            <Badge tone="brand" solid>
              Approved
            </Badge>
            <Meta>
              Merges right after <Mono>{waiting().parent}</Mono>, which it's
              stacked on, lands.
            </Meta>
            <span class={styles.actions}>
              <Show when={failure()}>
                {(failure) => <ErrorNote failure={failure()} />}
              </Show>
              <CancelMerge sending={sending()} onCancel={cancel} />
            </span>
          </section>
        )}
      </Match>
      <Match when={props.blocked.kind === "behind" && props.blocked}>
        {(behind) => (
          <section class={styles.notice} aria-label="Merge behind">
            <Badge tone="warning" solid>
              Behind
            </Badge>
            <Meta>
              Approved, but its diff didn't apply to <Mono>{props.target}</Mono>{" "}
              once <Mono>{behind().parent}</Mono> merged. Sent to the agent to
              rebase: it merges on this approval if its diff comes back
              unchanged.
            </Meta>
            <span class={styles.actions}>
              <Show when={failure()}>
                {(failure) => <ErrorNote failure={failure()} />}
              </Show>
              <CancelMerge sending={sending()} onCancel={cancel} />
            </span>
          </section>
        )}
      </Match>
      <Match when={props.blocked.kind === "conflict" && props.blocked}>
        {(conflict) => (
          <section class={styles.notice} aria-label="Merge conflict">
            <Badge tone="warning" solid>
              Conflict
            </Badge>
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

function CancelMerge(props: {
  sending: "cancel" | "retry" | undefined;
  onCancel: () => void;
}) {
  return (
    <Button
      variant="danger"
      disabled={props.sending !== undefined}
      busy={props.sending === "cancel"}
      onClick={() => props.onCancel()}
    >
      {props.sending === "cancel" ? "Cancelling…" : "Cancel merge"}
    </Button>
  );
}

function split(reason: string): { said: string; detail: string | undefined } {
  const at = reason.indexOf("\n\n");
  if (at < 0) return { said: reason, detail: undefined };
  return { said: reason.slice(0, at), detail: reason.slice(at + 2).trim() };
}
