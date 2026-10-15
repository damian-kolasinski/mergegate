import { createSignal, type JSX, Show } from "solid-js";
import { useModalKeys } from "../app/keys";
import styles from "./more.module.css";

/**
 * The header's `⋯`: actions taken rarely, kept off the line the operator
 * reads. The screen hears no keys while it's open, so `Escape` closes it
 * rather than leaving the item.
 */
export function MoreMenu(props: {
  /** The menu's contents; `close` folds it again. */
  children: (close: () => void) => JSX.Element;
}) {
  const [open, setOpen] = createSignal(false);
  let toggle: HTMLButtonElement | undefined;
  const close = () => {
    setOpen(false);
    toggle?.focus();
  };
  return (
    <span class={styles.more}>
      <button
        type="button"
        ref={toggle}
        class={styles.toggle}
        aria-label="More actions"
        aria-expanded={open()}
        aria-controls="more"
        title="More actions"
        onClick={() => setOpen(!open())}
      >
        ⋯
      </button>
      <Show when={open()}>
        <Panel onClose={close}>{props.children(close)}</Panel>
      </Show>
    </span>
  );
}

function Panel(props: { onClose: () => void; children: JSX.Element }) {
  useModalKeys({ Escape: () => props.onClose() });
  return (
    <>
      <div class={styles.scrim} onClick={() => props.onClose()} />
      <div
        id="more"
        class={styles.panel}
        role="group"
        aria-label="More actions"
      >
        {props.children}
      </div>
    </>
  );
}
