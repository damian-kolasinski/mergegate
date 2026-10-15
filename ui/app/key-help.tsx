import { createSignal, For, Show } from "solid-js";
import { activeKeyHelp, useKeys, useModalKeys } from "./keys";
import styles from "./key-help.module.css";

/**
 * `?` lists the keys of whatever is on screen, from the keymaps bound right
 * now, so the overlay can't drift from what the keys do.
 */
export function KeyHelpOverlay() {
  const [open, setOpen] = createSignal(false);
  useKeys({ "?": () => setOpen(true) });
  return (
    <Show when={open()}>
      <Overlay onClose={() => setOpen(false)} />
    </Show>
  );
}

function Overlay(props: { onClose: () => void }) {
  useModalKeys({
    Escape: () => props.onClose(),
    "?": () => props.onClose(),
  });
  return (
    <>
      <div class={styles.scrim} onClick={() => props.onClose()} />
      <div
        class={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
      >
        <h2 class={styles.title}>Keyboard shortcuts</h2>
        <For each={activeKeyHelp()}>
          {(help) => (
            <section class={styles.group} aria-label={help.group}>
              <h3 class={styles.heading}>{help.group}</h3>
              <dl class={styles.keys}>
                <For each={help.keys}>
                  {([key, does]) => (
                    <>
                      <dt>
                        <For each={key.split(" / ")}>
                          {(one, at) => (
                            <>
                              <Show when={at() > 0}> / </Show>
                              <kbd>{one}</kbd>
                            </>
                          )}
                        </For>
                      </dt>
                      <dd>{does}</dd>
                    </>
                  )}
                </For>
              </dl>
            </section>
          )}
        </For>
        <p class={styles.foot}>
          <kbd>?</kbd> or <kbd>Esc</kbd> closes this
        </p>
      </div>
    </>
  );
}
