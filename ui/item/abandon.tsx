import { createSignal, Show } from "solid-js";
import type { Abandon as AbandonBody } from "../api/Abandon";
import type { Item } from "../api/Item";
import { postJson } from "../app/hub";
import { Button, Meta, TextArea } from "../design-system";
import styles from "./abandon.module.css";

/**
 * The header's Abandon button, and the confirm step it opens: closing an
 * item drops its diffs, so it takes a second click and has no key.
 */
export function Abandon(props: {
  item: string;
  open: boolean;
  onToggle: () => void;
  onAbandoned: (item: Item) => void;
}) {
  const [reason, setReason] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [failure, setFailure] = createSignal<string>();

  const abandon = () => {
    const body: AbandonBody = {
      source: "operator",
      reason: reason().trim() || null,
    };
    setSending(true);
    setFailure(undefined);
    postJson<Item>(`/api/items/${encodeURIComponent(props.item)}/abandon`, body)
      .then(props.onAbandoned, (error: unknown) =>
        setFailure(error instanceof Error ? error.message : String(error)),
      )
      .finally(() => setSending(false));
  };

  return (
    <>
      <span class={styles.toggle}>
        <Button
          expanded={props.open}
          controls="abandon"
          onClick={() => props.onToggle()}
        >
          Abandon
        </Button>
      </span>
      <Show when={props.open}>
        <form id="abandon" class={styles.panel} aria-label="Abandon">
          <Meta>
            Close {props.item} without merging. Its agent hears why, and its
            diffs are dropped.
          </Meta>
          <TextArea
            label="Reason"
            placeholder="Why, for the agent (optional)"
            value={reason()}
            disabled={sending()}
            onInput={setReason}
            ref={(element) => requestAnimationFrame(() => element.focus())}
          />
          <div class={styles.actions}>
            <Show when={failure()}>
              {(message) => <Meta tone="removed">{message()}</Meta>}
            </Show>
            <Button disabled={sending()} onClick={() => props.onToggle()}>
              Cancel
            </Button>
            <Button variant="solid" disabled={sending()} onClick={abandon}>
              Abandon {props.item}
            </Button>
          </div>
        </form>
      </Show>
    </>
  );
}
