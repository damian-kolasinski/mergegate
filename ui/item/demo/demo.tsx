import { createSignal, Show } from "solid-js";
import type { Demo as DemoData } from "../../api/Demo";
import styles from "./demo.module.css";
import { Viewer } from "./viewer";

/**
 * The screenshots and recordings the agent attached, as a chip in the
 * header that opens them over the review: the diff keeps its height. They
 * may come from an earlier revision when the newest has none, which the
 * chip says.
 */
export function Demo(props: { demo: DemoData; revision: number }) {
  const [shown, setShown] = createSignal<number>();
  let chip: HTMLButtonElement | undefined;
  const close = () => {
    setShown(undefined);
    chip?.focus();
  };
  const older = () => props.demo.revision !== props.revision;
  const count = () => props.demo.attachments.length;

  return (
    <>
      <button
        type="button"
        ref={chip}
        class={styles.chip}
        title={`${count()} ${count() === 1 ? "attachment" : "attachments"} from r${props.demo.revision}${older() ? `, not r${props.revision}` : ""}`}
        onClick={() => setShown(0)}
      >
        <span class={styles.play} aria-hidden="true">
          ▶
        </span>
        Demo ({count()})
        <Show when={older()}>
          <span class={styles.from}>from r{props.demo.revision}</span>
        </Show>
      </button>
      <Show when={shown() !== undefined}>
        <Viewer
          attachments={props.demo.attachments}
          index={shown() ?? 0}
          onStep={setShown}
          onClose={close}
        />
      </Show>
    </>
  );
}
