import type { JSX } from "solid-js";
import styles from "./badge.module.css";

export type Tone = "neutral" | "brand" | "success" | "warning" | "error";

/** A short label on a solid fill: a state, a revision, a count. */
export function Badge(props: { tone?: Tone; children: JSX.Element }) {
  return (
    <span class={styles.badge} data-tone={props.tone ?? "neutral"}>
      {props.children}
    </span>
  );
}
