import { For, Show } from "solid-js";
import type { StackEntry } from "../api/StackEntry";
import { itemHref } from "../app/router";
import { Badge, Meta, Mono } from "../design-system";
import styles from "./stack.module.css";

/**
 * The stack the item is in, bottom first: the items it's built on, which
 * land before it, then those built on it. Its diff is its own, against the
 * one below.
 */
export function StackTrail(props: {
  item: string;
  stack: readonly StackEntry[];
  /** Built on a parent revision that's no longer the newest, or closed. */
  behind: boolean;
}) {
  return (
    <nav class={styles.stack} aria-label="Stack">
      <Meta>Stack</Meta>
      <ol class={styles.entries}>
        <For each={props.stack}>
          {(entry) => (
            <li class={styles.entry}>
              <Show
                when={entry.id !== props.item}
                fallback={
                  <span class={styles.current} aria-current="page">
                    <Mono>{entry.name}</Mono> {entry.title}
                  </span>
                }
              >
                <a class={styles.link} href={itemHref(entry.id)}>
                  <Mono>{entry.name}</Mono> {entry.title}
                </a>
              </Show>
              <Show when={entry.state === "merged"}>
                <Badge tone="success">merged</Badge>
              </Show>
              <Show when={entry.state === "abandoned"}>
                <Badge tone="neutral">abandoned</Badge>
              </Show>
              <Show when={entry.id === props.item && props.behind}>
                <Badge
                  tone="warning"
                  title="Built on an older revision of the item below: its agent was told to rebase"
                >
                  behind
                </Badge>
              </Show>
            </li>
          )}
        </For>
      </ol>
    </nav>
  );
}
