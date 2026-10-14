import { createSignal, Show } from "solid-js";
import type { SubmitReview } from "../api/SubmitReview";
import type { Verdict } from "../api/Verdict";
import { postJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { Button, Meta, TextArea } from "../design-system";
import styles from "./review-bar.module.css";

/**
 * Sends the operator's review of one revision to its agent. Comment leaves
 * the item with the operator; Request changes hands it back.
 */
export function ReviewBar(props: {
  item: string;
  revision: number;
  onSubmitted: () => void;
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
    postJson(`/api/items/${encodeURIComponent(props.item)}/reviews`, body)
      .then(
        () => {
          setSummary("");
          setStatus({
            text: `Sent to the agent on r${body.revision}`,
            failed: false,
          });
          props.onSubmitted();
        },
        (error: unknown) =>
          setStatus({
            text: error instanceof Error ? error.message : String(error),
            failed: true,
          }),
      )
      .finally(() => setSending(false));
  };
  const empty = () => !summary().trim();

  useKeys({ r: () => field?.focus() });

  return (
    <form
      class={styles.bar}
      aria-label="Review"
      onSubmit={(event) => event.preventDefault()}
    >
      <TextArea
        label="Review summary"
        placeholder="Summary for the agent (r)"
        value={summary()}
        onInput={setSummary}
        ref={(element) => {
          field = element;
        }}
      />
      <div class={styles.actions}>
        <Show when={status()}>
          {(current) => (
            <Meta tone={current().failed ? "removed" : "subtle"}>
              {current().text}
            </Meta>
          )}
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
