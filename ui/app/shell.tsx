import { type JSX, Show } from "solid-js";
import { hubConnected } from "./hub";
import { KeyHelpOverlay } from "./key-help";
import { OpenOnPhone } from "./phone";
import styles from "./shell.module.css";
import { HubVersion } from "./version";

/**
 * The frame every screen renders in. A `fill` screen takes the whole window
 * below the bar and scrolls its own panes, rather than the page. `trail` is
 * where the screen is, after the name: "Inbox / app#42".
 */
export function Shell(props: {
  fill?: boolean;
  trail?: JSX.Element;
  children: JSX.Element;
}) {
  return (
    <div class={styles.shell} data-fill={props.fill ? "" : undefined}>
      <header class={styles.bar}>
        <a class={styles.name} href="#/">
          <img src="/assets/favicon.svg" width="20" height="20" alt="" />
          mergegate
        </a>
        <HubVersion />
        <Show when={props.trail}>
          <nav class={styles.trail} aria-label="Breadcrumb">
            {props.trail}
          </nav>
        </Show>
        <Show when={!hubConnected()}>
          <span class={styles.offline} role="status">
            <span class={styles.pulse} aria-hidden="true" />
            Reconnecting to hub… what's shown may be out of date
          </span>
        </Show>
        <span class={styles.help}>
          <kbd>?</kbd> shortcuts
        </span>
        <OpenOnPhone />
      </header>
      <main class={styles.main}>{props.children}</main>
      <KeyHelpOverlay />
    </div>
  );
}
