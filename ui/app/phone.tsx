import { createSignal, Show } from "solid-js";
import type { Settings } from "../api/Settings";
import { Button } from "../design-system";
import { getJson, onHubEvent } from "./hub";
import { useModalKeys } from "./keys";
import styles from "./phone.module.css";

/**
 * While the hub listens on the tailnet, a button that shows this page's
 * tailnet link as a QR code, so the operator carries on from a phone on the
 * tailnet where they are. Shows nothing otherwise: no phone reaches the hub.
 */
export function OpenOnPhone() {
  const [tailnet, setTailnet] = createSignal<string>();
  const [open, setOpen] = createSignal(false);
  const load = () =>
    getJson<Settings>("/api/hub/settings").then(
      (settings) => setTailnet(settings.tailnet?.url),
      () => undefined,
    );
  void load();
  // A reconnect may be to a hub restarted with another listen setting.
  onHubEvent("inbox.changed", (event) => {
    if (!event) void load();
  });
  return (
    <Show when={tailnet()}>
      {(url) => (
        <>
          <span class={styles.trigger}>
            <Button
              title="Show a QR code of this page for a phone on the tailnet"
              expanded={open()}
              onClick={() => setOpen(true)}
            >
              Open on phone
            </Button>
          </span>
          <Show when={open()}>
            <Dialog tailnet={url()} onClose={() => setOpen(false)} />
          </Show>
        </>
      )}
    </Show>
  );
}

function Dialog(props: { tailnet: string; onClose: () => void }) {
  useModalKeys({ Escape: () => props.onClose() });
  // The page as it was when opened: the code shouldn't change under a camera.
  const route = location.hash || "#/";
  const link = () => `${props.tailnet}/${route}`;
  return (
    <>
      <div class={styles.scrim} onClick={() => props.onClose()} />
      <div
        class={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label="Open on your phone"
      >
        <h2 class={styles.title}>Open on your phone</h2>
        <img
          class={styles.code}
          src={`/api/hub/qr?route=${encodeURIComponent(route)}`}
          alt={`QR code of ${link()}`}
          width="240"
          height="240"
        />
        <a class={styles.link} href={link()}>
          {link()}
        </a>
        <p class={styles.foot}>
          Scan it with a phone on the tailnet to open this page there.{" "}
          <kbd>Esc</kbd> closes this.
        </p>
      </div>
    </>
  );
}
