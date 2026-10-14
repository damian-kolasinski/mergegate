import { Show } from "solid-js";
import { Meta } from "../design-system";
import styles from "./counts.module.css";

/** `+N −N`, leaving out a zero or a count a non-text change doesn't have. */
export function Counts(props: {
  additions: number | null;
  deletions: number | null;
}) {
  return (
    <span class={styles.counts}>
      <Show when={props.additions}>
        <Meta tone="added">+{props.additions}</Meta>
      </Show>
      <Show when={props.deletions}>
        <Meta tone="removed">−{props.deletions}</Meta>
      </Show>
    </span>
  );
}
