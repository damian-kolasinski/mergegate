import { onCleanup, onMount, Show } from "solid-js";
import type { Attachment } from "../../api/Attachment";
import { Button, Meta } from "../../design-system";
import styles from "./demo.module.css";

/**
 * One attachment at full size over the review. Escape closes it and the
 * arrows step through the rest.
 */
export function Viewer(props: {
  attachments: Attachment[];
  index: number;
  onStep: (index: number) => void;
  onClose: () => void;
}) {
  const current = () => props.attachments[props.index];
  const step = (by: number) => {
    const next = props.index + by;
    if (next >= 0 && next < props.attachments.length) props.onStep(next);
  };
  let close: HTMLButtonElement | undefined;

  // Captured, so it runs before the review's keys, which skip a key whose
  // default is prevented: Escape here closes the viewer rather than leaving
  // for the inbox, and the review's letter shortcuts wait until it's closed.
  // Space and Tab stay with the player's controls.
  const onKey = (event: KeyboardEvent) => {
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const actions: Record<string, () => void> = {
      Escape: props.onClose,
      ArrowLeft: () => step(-1),
      ArrowRight: () => step(1),
    };
    const action = actions[event.key];
    if (action) {
      event.preventDefault();
      action();
    } else if (event.key.length === 1 && event.key !== " ") {
      event.preventDefault();
    }
  };
  onMount(() => {
    window.addEventListener("keydown", onKey, { capture: true });
    close?.focus();
  });
  onCleanup(() =>
    window.removeEventListener("keydown", onKey, { capture: true }),
  );

  return (
    <div class={styles.scrim} onClick={() => props.onClose()}>
      <div
        class={styles.viewer}
        role="dialog"
        aria-modal="true"
        aria-label={current()?.name}
        onClick={(event) => event.stopPropagation()}
      >
        <div class={styles.bar}>
          <span class={styles.name}>{current()?.name}</span>
          <Meta>
            {props.index + 1} / {props.attachments.length}
          </Meta>
          <span class={styles.actions}>
            <Button disabled={props.index === 0} onClick={() => step(-1)}>
              ←
            </Button>
            <Button
              disabled={props.index === props.attachments.length - 1}
              onClick={() => step(1)}
            >
              →
            </Button>
            <Button
              onClick={() => props.onClose()}
              ref={(element) => {
                close = element;
              }}
            >
              Close
            </Button>
          </span>
        </div>
        <Show when={current()} keyed>
          {(attachment) => (
            <Show
              when={attachment.video}
              fallback={
                <img
                  class={styles.full}
                  src={attachment.url}
                  alt={attachment.name}
                />
              }
            >
              <video
                class={styles.full}
                src={attachment.url}
                aria-label={attachment.name}
                controls
                autoplay
                muted
                loop
              />
            </Show>
          )}
        </Show>
      </div>
    </div>
  );
}
