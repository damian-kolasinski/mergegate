import { Show, type JSX } from "solid-js";
import styles from "./button.module.css";

/**
 * An action. `solid` is the one a screen leads with, `danger` one that throws
 * work away; the rest are `outline`. While `busy` the button stays disabled
 * but says so, so a slow action doesn't look like a forbidden one.
 */
export function Button(props: {
  variant?: "solid" | "outline" | "danger";
  disabled?: boolean | undefined;
  busy?: boolean | undefined;
  /** Why it's disabled, or what it does; shown on hover. */
  title?: string | undefined;
  /** The shortcut that does the same, shown after the label. */
  keyHint?: string | undefined;
  onClick: () => void;
  /** For a button that opens a panel: whether it's open, and the panel's id. */
  expanded?: boolean;
  controls?: string;
  ref?: (element: HTMLButtonElement) => void;
  children: JSX.Element;
}) {
  return (
    <button
      type="button"
      class={styles.button}
      data-variant={props.variant ?? "outline"}
      disabled={props.disabled || props.busy}
      aria-busy={props.busy ? "true" : undefined}
      aria-expanded={props.expanded}
      aria-controls={props.controls}
      aria-keyshortcuts={props.keyHint}
      title={props.title}
      ref={(element) => props.ref?.(element)}
      onClick={() => props.onClick()}
    >
      {props.children}
      <Show when={props.keyHint}>
        {(key) => <kbd class={styles.key}>{key()}</kbd>}
      </Show>
    </button>
  );
}
