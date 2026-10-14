import { createSignal, For, Show } from "solid-js";
import type { Attachment } from "../../api/Attachment";
import type { Demo as DemoData } from "../../api/Demo";
import { Meta } from "../../design-system";
import styles from "./demo.module.css";
import { Viewer } from "./viewer";

/**
 * The screenshots and recordings the agent attached, as a strip under the
 * header, to watch before reading the diff. They may come from an earlier
 * revision when the newest has none, which the strip says.
 */
export function Demo(props: { demo: DemoData; revision: number }) {
  const [shown, setShown] = createSignal<number>();
  const thumbs: HTMLButtonElement[] = [];
  const close = () => {
    const index = shown();
    setShown(undefined);
    if (index !== undefined) thumbs[index]?.focus();
  };

  return (
    <section class={styles.strip} aria-label="Demo">
      <span class={styles.label}>Demo</span>
      <Show when={props.demo.revision !== props.revision}>
        <Meta>from r{props.demo.revision}</Meta>
      </Show>
      <ul class={styles.thumbs}>
        <For each={props.demo.attachments}>
          {(attachment, index) => (
            <li>
              <button
                type="button"
                class={styles.thumb}
                title={attachment.name}
                aria-label={`Open ${attachment.name}`}
                ref={(element) => {
                  thumbs[index()] = element;
                }}
                onClick={() => setShown(index())}
              >
                <Preview attachment={attachment} />
                <Show when={attachment.video}>
                  <span class={styles.play} aria-hidden="true">
                    ▶
                  </span>
                </Show>
              </button>
            </li>
          )}
        </For>
      </ul>
      <Show when={shown() !== undefined}>
        <Viewer
          attachments={props.demo.attachments}
          index={shown() ?? 0}
          onStep={setShown}
          onClose={close}
        />
      </Show>
    </section>
  );
}

/** A video's first frame, or the image itself scaled down. */
function Preview(props: { attachment: Attachment }) {
  return (
    <Show
      when={props.attachment.video}
      fallback={
        <img
          class={styles.media}
          src={props.attachment.url}
          alt=""
          loading="lazy"
        />
      }
    >
      {/* `#t` makes Safari paint a frame instead of a blank box. */}
      <video
        class={styles.media}
        src={`${props.attachment.url}#t=0.1`}
        preload="metadata"
        muted
      />
    </Show>
  );
}
