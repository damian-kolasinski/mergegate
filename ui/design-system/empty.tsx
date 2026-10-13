import type { JSX } from "solid-js";
import styles from "./empty.module.css";

/** Stands in for a list with nothing in it, saying why or what comes next. */
export function Empty(props: { children: JSX.Element }) {
  return <p class={styles.empty}>{props.children}</p>;
}
