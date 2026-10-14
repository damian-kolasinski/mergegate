import type { JSX } from "solid-js";
import styles from "./button.module.css";

/** An action. `solid` is the one a screen leads with; the rest are `outline`. */
export function Button(props: {
  variant?: "solid" | "outline";
  disabled?: boolean;
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
      disabled={props.disabled}
      aria-expanded={props.expanded}
      aria-controls={props.controls}
      ref={(element) => props.ref?.(element)}
      onClick={() => props.onClick()}
    >
      {props.children}
    </button>
  );
}
