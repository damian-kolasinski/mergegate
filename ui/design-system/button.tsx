import type { JSX } from "solid-js";
import styles from "./button.module.css";

/** An action. `solid` is the one a screen leads with; the rest are `outline`. */
export function Button(props: {
  variant?: "solid" | "outline";
  disabled?: boolean;
  onClick: () => void;
  children: JSX.Element;
}) {
  return (
    <button
      type="button"
      class={styles.button}
      data-variant={props.variant ?? "outline"}
      disabled={props.disabled}
      onClick={() => props.onClick()}
    >
      {props.children}
    </button>
  );
}
