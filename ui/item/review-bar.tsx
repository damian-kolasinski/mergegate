import { createSignal, Show } from "solid-js";
import type { Review } from "../api/Review";
import type { SubmitReview } from "../api/SubmitReview";
import type { Verdict } from "../api/Verdict";
import { postJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { Button, Meta, TextArea } from "../design-system";
import styles from "./review-bar.module.css";

/**
 * Sends the operator's review of one revision to its agent, with every draft
 * comment on the item. Comment leaves the item with the operator; Request
 * changes hands it back.
 */
export function ReviewBar(props: {
  item: string;
  revision: number;
  drafts: number;
  onSubmitted: (review: Review) => void;
}) {
  const [summary, setSummary] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [status, setStatus] = createSignal<{ text: string; failed: boolean }>();
  let field: HTMLTextAreaElement | undefined;

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
      .then(
        (review) => {
          setSummary("");
          const count = review.comments.length;
          setStatus({
            text:
              `Sent to the agent on r${body.revision}` +
              (count ? ` with ${plural(count, "comment")}` : ""),
            failed: false,
          });
          props.onSubmitted(review);
        },
        (error: unknown) =>
          setStatus({
            text: error instanceof Error ? error.message : String(error),
            failed: true,
          }),
      )
      .finally(() => setSending(false));
  };
  const empty = () => !summary().trim() && !props.drafts;

  useKeys({ r: () => field?.focus() });

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
        <Button
          disabled={sending() || empty()}
          onClick={() => submit("comment")}
        >
          Comment
        </Button>
        <Button
          variant="solid"
          disabled={sending() || empty()}
          onClick={() => submit("changes_requested")}
        >
          Request changes
        </Button>
      </div>
    </form>
  );
}

function plural(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? "" : "s"}`;
}
