import { For, type JSX, Show } from "solid-js";
import styles from "./text.module.css";

/** Secondary detail next to a primary line: ages, counts, branches. */
export function Meta(props: {
  tone?: "subtle" | "added" | "removed";
  children: JSX.Element;
}) {
  return (
    <span class={styles.meta} data-tone={props.tone ?? "subtle"}>
      {props.children}
    </span>
  );
}

/** An identifier or code: ids, branches, paths. */
export function Mono(props: { children: JSX.Element }) {
  return <code class={styles.mono}>{props.children}</code>;
}

/**
 * Prose that marks code the markdown way, `like this`: the marked parts
 * render as code instead of showing their backticks. Text only, so nothing
 * in it can run or restyle the page.
 */
export function CodeText(props: { text: string }) {
  return (
    <For each={marked(props.text, /`([^`]+)`/)}>
      {(part) => (
        <Show when={part.marked} fallback={part.text}>
          <code class={styles.code}>{part.text}</code>
        </Show>
      )}
    </For>
  );
}

/**
 * `text` cut where `mark` matches, each part saying whether the match's
 * first group is what it holds.
 */
export function marked(
  text: string,
  mark: RegExp,
): { text: string; marked: boolean }[] {
  // Splitting on a capture group puts the marked parts at the odd indexes.
  return text
    .split(mark)
    .map((part, index) => ({ text: part, marked: index % 2 === 1 }));
}
