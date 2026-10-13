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
    if (props.selected) row?.scrollIntoView({ block: "nearest" });
  });
  return (
    <li
      ref={row}
      class={styles.row}
      aria-current={props.selected ? "true" : undefined}
      onClick={() => {
        props.onSelect();
        props.onOpen();
      }}
    >
      {props.children}
    </li>
  );
}
