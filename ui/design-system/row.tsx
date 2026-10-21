import { createEffect, type JSX } from "solid-js";
import styles from "./row.module.css";

/**
 * One entry of a keyboard-driven list. The list keeps the selection (moved
 * with `j/k`), so a row marks it rather than taking DOM focus.
 */
export function Row(props: {
  selected: boolean;
  onSelect: () => void;
  onOpen: () => void;
  children: JSX.Element;
}) {
  let row: HTMLLIElement | undefined;
  createEffect(() => {
    if (!props.selected) return;
    // Once the update that selected it has settled, as it may also lay out
    // what the row is scrolled clear of, such as the file list's stuck
    // headers over a group it unfolded. Still before the next paint.
    queueMicrotask(() => row?.scrollIntoView({ block: "nearest" }));
  });
  return (
    <li
      ref={row}
      class={styles.row}
      aria-current={props.selected ? "true" : undefined}
      onClick={(event) => {
        // A link in the row goes where it points, and a modified click
        // (a new tab, say) stays with the browser.
        if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
          return;
        }
        props.onSelect();
        if (!(event.target as Element).closest("a")) props.onOpen();
      }}
    >
      {props.children}
    </li>
  );
}
