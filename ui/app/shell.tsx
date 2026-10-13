import type { JSX } from "solid-js";
import styles from "./shell.module.css";

/** The frame every screen renders in. */
export function Shell(props: { children: JSX.Element }) {
  return (
    <div class={styles.shell}>
      <header class={styles.bar}>
        <a class={styles.name} href="#/">
          mergegate
        </a>
      </header>
      <main class={styles.main}>{props.children}</main>
    </div>
  );
}
