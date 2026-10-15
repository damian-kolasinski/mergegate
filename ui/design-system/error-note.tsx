import { Show } from "solid-js";
import type { Failure } from "../app/hub";
import styles from "./error-note.module.css";

/**
 * Something that failed, and what fixes it when that's known. Its own tone,
 * so it can't be mistaken for a deleted line or a count.
 */
export function ErrorNote(
  props: { failure: Failure } | { message: string; hint?: string | undefined },
) {
  const failure = (): Failure =>
    "failure" in props
      ? props.failure
      : { message: props.message, hint: props.hint };
  return (
    <span class={styles.note} role="alert">
      <span class={styles.icon} aria-hidden="true">
        !
      </span>
      <span>
        {failure().message}
        <Show when={failure().hint}>
          {(hint) => <span class={styles.hint}> — {hint()}</span>}
        </Show>
      </span>
    </span>
  );
}
