import type { JSX } from "solid-js";
import styles from "./shell.module.css";

/**
 * The frame every screen renders in. A `fill` screen takes the whole window
 * below the bar and scrolls its own panes, rather than the page.
 */
export function Shell(props: { fill?: boolean; children: JSX.Element }) {
  return (
    <div class={styles.shell} data-fill={props.fill ? "" : undefined}>
      <header class={styles.bar}>
        <a class={styles.name} href="#/">
          mergegate
        </a>
      </header>
      <main class={styles.main}>{props.children}</main>
    </div>
  );
}
