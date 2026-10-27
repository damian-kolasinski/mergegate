import { createSignal, Show } from "solid-js";
import type { Abandon as AbandonBody } from "../api/Abandon";
import type { Item } from "../api/Item";
import { type Failure, failureOf, HubError, postJson } from "../app/hub";
import { Button, ErrorNote, Meta, Mono, TextArea } from "../design-system";
import styles from "./abandon.module.css";
import { leaveReview } from "./review-time";

/**
 * The confirm step the menu's Abandon opens: closing an item drops its
 * diffs, so it takes a second click and has no key of its own.
 */
export function Abandon(props: {
  item: string;
  /** What the operator calls it, as in `Item`. */
  name: string;
  /** Set for an item on GitHub, whose pull request closes with it. */
  pullRequest: { label: string; branch: string } | undefined;
  /** What's typed as the reason, which a reload keeps. */
  reason: string;
  onReason: (reason: string) => void;
  onCancel: () => void;
  onAbandoned: (item: Item) => void;
}) {
  const [sending, setSending] = createSignal(false);
  // Which of the two abandons is being sent.
  const [leaving, setLeaving] = createSignal(false);
  const [failure, setFailure] = createSignal<Failure>();
  // GitHub can't close the pull request, as when the repository is gone:
  // the operator may close the item here alone instead.
  const [unreachable, setUnreachable] = createSignal(false);

  const abandon = (leaveGithub = false) => {
    if (sending()) return;
    const body: AbandonBody = {
      source: "operator",
      reason: props.reason.trim() || null,
      leave_github: leaveGithub,
    };
    setSending(true);
    setLeaving(leaveGithub);
    setFailure(undefined);
    const url = `/api/items/${encodeURIComponent(props.item)}/abandon`;
    // The time up to the abandon counts, and the hub takes none after it.
    leaveReview(() => postJson<Item>(url, body))
      .then(props.onAbandoned, (error: unknown) => {
        setFailure(failureOf(error));
        setUnreachable(
          error instanceof HubError && error.code === "github_no_access",
        );
      })
      .finally(() => setSending(false));
  };

  return (
    <form id="abandon" class={styles.panel} aria-label="Abandon">
      <Meta>
        Close {props.name} without merging. Its agent hears why, and its diffs
        are dropped.
      </Meta>
      <Show when={props.pullRequest}>
        {(pull) => (
          <Meta>
            Also closes PR {pull().label} on GitHub and deletes its branch,{" "}
            <Mono>{pull().branch}</Mono>.
          </Meta>
        )}
      </Show>
      <TextArea
        label="Reason"
        placeholder="Why, for the agent (optional) — ⌘↩ abandons"
        value={props.reason}
        disabled={sending()}
        onInput={(reason) => props.onReason(reason)}
        onSubmit={() => abandon()}
        ref={(element) => requestAnimationFrame(() => element.focus())}
      />
      <Show when={unreachable() && props.pullRequest}>
        {(pull) => (
          <Meta>
            If GitHub no longer has the repository, abandon {props.name} here
            only: PR {pull().label} and <Mono>{pull().branch}</Mono> stay as
            they are on GitHub.
          </Meta>
        )}
      </Show>
      <div class={styles.actions}>
        <Show when={failure()}>
          {(failure) => <ErrorNote failure={failure()} />}
        </Show>
        <Button disabled={sending()} onClick={() => props.onCancel()}>
          Cancel
        </Button>
        <Show when={unreachable()}>
          <Button
            variant="danger"
            disabled={sending()}
            busy={sending() && leaving()}
            onClick={() => abandon(true)}
          >
            {sending() && leaving()
              ? "Abandoning…"
              : "Abandon in mergegate only"}
          </Button>
        </Show>
        <Button
          variant="danger"
          disabled={sending()}
          busy={sending() && !leaving()}
          keyHint="⌘↩"
          onClick={() => abandon()}
        >
          {sending() && !leaving() ? "Abandoning…" : `Abandon ${props.name}`}
        </Button>
      </div>
    </form>
  );
}
