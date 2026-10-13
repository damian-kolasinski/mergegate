import { type JSX, Show } from "solid-js";
import styles from "./section.module.css";

/** A titled group of a screen, its count beside the title. */
export function Section(props: {
  title: string;
  count?: number;
  children: JSX.Element;
}) {
  return (
    <section class={styles.section} aria-label={props.title}>
      <h2 class={styles.header}>
        {props.title}
        <Show when={props.count !== undefined}>
          <span class={styles.count}>{props.count}</span>
        </Show>
      </h2>
      {props.children}
    </section>
  );
}
