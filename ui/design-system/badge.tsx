import type { JSX } from "solid-js";
import styles from "./badge.module.css";

export type Tone = "neutral" | "brand" | "success" | "warning" | "error";

/**
 * A short label: a state, a revision, a count. Tinted by default; `solid` is
 * for the few states that need the operator now (p0, a conflict, a held
 * merge, a rejection), so they outweigh everything else on a row.
 */
export function Badge(props: {
  tone?: Tone;
  solid?: boolean;
  title?: string | undefined;
  children: JSX.Element;
}) {
  return (
    <span
      class={styles.badge}
      data-tone={props.tone ?? "neutral"}
      data-solid={props.solid ? "" : undefined}
      title={props.title}
    >
      <span class={styles.label}>{props.children}</span>
    </span>
  );
}
