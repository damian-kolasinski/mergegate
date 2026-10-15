import { Show } from "solid-js";
import type { Item } from "../api/Item";
import { Mono } from "../design-system";
import styles from "./closed.module.css";
import { NextItem } from "./next";

/**
 * Over what's left of a closed item: that it landed, or that it was
 * dropped, and the item that needs the operator next.
 */
export function Closed(props: { item: Item }) {
  const merged = () => props.item.state === "merged";
  return (
    <section
      class={styles.closed}
      data-merged={merged() ? "" : undefined}
      aria-label={merged() ? "Merged" : "Abandoned"}
    >
      <Show
        when={merged()}
        fallback={<span class={styles.said}>Abandoned</span>}
      >
        <svg
          class={styles.check}
          viewBox="0 0 24 24"
          width="24"
          height="24"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="11" />
          <path d="M7 12.5l3.2 3.2L17 9" />
        </svg>
        <span class={styles.said}>
          Merged into <Mono>{props.item.target}</Mono>
        </span>
      </Show>
      <NextItem after={props.item.id} />
    </section>
  );
}
