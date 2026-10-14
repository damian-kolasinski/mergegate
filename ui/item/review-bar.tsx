import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import type { Approve } from "../api/Approve";
import type { Merge } from "../api/Merge";
import type { Review } from "../api/Review";
import type { SubmitReview } from "../api/SubmitReview";
import type { Verdict } from "../api/Verdict";
import { postJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { Button, Meta, Mono, TextArea } from "../design-system";
import styles from "./review-bar.module.css";

/**
 * Sends the operator's review of one revision to its agent, with every draft
 * comment and reply on the item. Comment leaves the item with the operator;
 * Request changes hands it back; Approve & merge lands it on the target, after
 * a confirm step.
 */
export function ReviewBar(props: {
  item: string;
  revision: number;
  target: string;
  /** Files not marked viewed, which the confirm step warns about. */
  unviewed: number;
  drafts: number;
  /** Draft replies on sent comments. */
  replies: number;
  /** A comment being written, which the review would go out without. */
  holding: boolean;
  onSubmitted: (review: Review) => void;
  onMerged: (merge: Merge) => void;
}) {
  const [summary, setSummary] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [status, setStatus] = createSignal<{ text: string; failed: boolean }>();
  const [confirming, setConfirming] = createSignal(false);
  let field: HTMLTextAreaElement | undefined;

  const failed = (error: unknown) =>
    setStatus({
      text: error instanceof Error ? error.message : String(error),
      failed: true,
    });

  const submit = (verdict: Verdict) => {
    const body: SubmitReview = {
      revision: props.revision,
      verdict,
      summary: summary().trim(),
    };
    setSending(true);
    setStatus(undefined);
    postJson<Review>(
      `/api/items/${encodeURIComponent(props.item)}/reviews`,
      body,
    )
      .then((review) => {
        setSummary("");
        const parts = [
          review.comments.length && plural(review.comments.length, "comment"),
          review.replies.length && plural(review.replies.length, "reply"),
        ].filter(Boolean);
        setStatus({
          text:
            `Sent to the agent on r${body.revision}` +
            (parts.length ? ` with ${parts.join(" and ")}` : ""),
          failed: false,
        });
        props.onSubmitted(review);
      }, failed)
      .finally(() => setSending(false));
  };
  const empty = () => !summary().trim() && !props.drafts && !props.replies;

  const approve = () => {
    const body: Approve = {
      revision: props.revision,
      summary: summary().trim(),
    };
    setConfirming(false);
    setSending(true);
    setStatus(undefined);
    postJson<Merge>(
      `/api/items/${encodeURIComponent(props.item)}/approve`,
      body,
    )
      .then((merge) => {
        setSummary("");
        props.onMerged(merge);
      }, failed)
      .finally(() => setSending(false));
  };
  const confirm = () => {
    if (!sending() && !props.holding) setConfirming(true);
  };

  useKeys({ r: () => field?.focus(), a: confirm });
  // Escape backs out of the confirm step rather than leaving the item; the
  // screen binds its Escape first, so this one listens ahead of it.
  createEffect(() => {
    if (!confirming()) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      setConfirming(false);
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  return (
    <form class={styles.bar} aria-label="Review">
      <div class={styles.summary}>
        <TextArea
          label="Review summary"
          placeholder="Summary for the agent (r)"
          value={summary()}
          // What's typed while sending would be cleared with the rest.
          disabled={sending()}
          onInput={setSummary}
          ref={(element) => {
            field = element;
          }}
        />
      </div>
      <div class={styles.actions}>
        <p role="status" class={styles.status}>
          <Show when={status()}>
            {(current) => (
              <Meta tone={current().failed ? "removed" : "subtle"}>
                {current().text}
              </Meta>
            )}
          </Show>
        </p>
        <Show when={props.drafts}>
          <Meta>{plural(props.drafts, "draft comment")}</Meta>
        </Show>
        <Show when={props.replies}>
          <Meta>{plural(props.replies, "draft reply")}</Meta>
        </Show>
        <Show when={props.holding}>
          <Meta>Save or cancel the open comment first</Meta>
        </Show>
        <Button
          disabled={sending() || empty() || props.holding}
          onClick={() => submit("comment")}
        >
          Comment
        </Button>
        <Show
          when={confirming()}
          fallback={
            <>
              <Button
                disabled={sending() || empty() || props.holding}
                onClick={() => submit("changes_requested")}
              >
                Request changes
              </Button>
              <Button
                variant="solid"
                disabled={sending() || props.holding}
                onClick={confirm}
              >
                Approve & merge
              </Button>
            </>
          }
        >
          <fieldset class={styles.confirm} aria-label="Confirm merge">
            <Meta>
              Squash r{props.revision} onto <Mono>{props.target}</Mono>?
            </Meta>
            <Show when={props.unviewed}>
              <Meta tone="removed">
                {plural(props.unviewed, "file")} unviewed
              </Meta>
            </Show>
            <Button onClick={() => setConfirming(false)}>Cancel</Button>
            <Button
              variant="solid"
              onClick={approve}
              ref={(element) => requestAnimationFrame(() => element.focus())}
            >
              Merge
            </Button>
          </fieldset>
        </Show>
      </div>
    </form>
  );
}

function plural(count: number, noun: string): string {
  if (count === 1) return `${count} ${noun}`;
  return `${count} ${noun.endsWith("y") ? `${noun.slice(0, -1)}ies` : `${noun}s`}`;
}
