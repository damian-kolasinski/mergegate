import type { JSX } from "solid-js";
import styles from "./notice.module.css";

/**
 * A strip under the header for what ends the review or stands in the way
 * of its merge: one line where it fits, tinted by what it says, with what
 * the operator can do about it at the end. Notes that only inform are
 * `Note`s instead, which keep to one line with each other.
 */
export function Notice(props: {
  label: string;
  tone: "warning" | "success" | "neutral";
  /** At the end of the line. */
  actions?: JSX.Element;
  children: JSX.Element;
}) {
  return (
    <section
      class={styles.notice}
      data-tone={props.tone}
      aria-label={props.label}
    >
      {props.children}
      <span class={styles.actions}>{props.actions}</span>
    </section>
  );
}

/**
 * A note that only informs, on a line with the others: in the review bar's
 * status row, or under the header while there's no review bar.
 */
export function Note(props: { label: string; children: JSX.Element }) {
  return (
    <section class={styles.note} aria-label={props.label}>
      {props.children}
    </section>
  );
}
