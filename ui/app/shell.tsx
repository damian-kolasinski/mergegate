import { type JSX, Show } from "solid-js";
import { hubConnected } from "./hub";
import { KeyHelpOverlay } from "./key-help";
import styles from "./shell.module.css";

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
          <Logo />
          mergegate
        </a>
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
      </header>
      <main class={styles.main}>{props.children}</main>
      <KeyHelpOverlay />
    </div>
  );
}

/** Two branches meeting at a gate: the mark beside the name. */
function Logo() {
  return (
    <svg
      class={styles.logo}
      viewBox="0 0 20 20"
      width="20"
      height="20"
      aria-hidden="true"
    >
      <rect width="20" height="20" rx="5" fill="currentColor" />
      <path
        d="M6 4.5v4.2c0 1.6 1.3 2.8 2.8 2.8H10m4-7v4.2c0 1.6-1.3 2.8-2.8 2.8H10m0 0v4"
        fill="none"
        stroke="var(--background-page)"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  );
}
