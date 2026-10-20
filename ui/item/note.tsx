import { createSignal, onCleanup, onMount, Show } from "solid-js";
import type { File } from "../api/File";
import { useFirstKeys } from "../app/keys";
import { CodeText } from "../design-system";
import styles from "./note.module.css";
import { importance } from "./why";

/** Whether `file` has anything for its note to say. */
export function noted(file: File): boolean {
  return Boolean(
    file.reason || file.importance !== null || file.why || file.hint,
  );
}

/**
 * Why a file is where it is, beside the list rather than in it: the guide's
 * reason, the judge's say, the rule that tiered it. Kept out of the list,
 * which then has room to show where each file lives; opened from the file's
 * ⓘ or with `i`, and closed again with either, `Escape`, or a click
 * elsewhere.
 */
export function Note(props: {
  file: File;
  /** The row it's beside, which it follows as the list scrolls. */
  anchor: HTMLElement;
  onClose: () => void;
}) {
  let note: HTMLDivElement | undefined;
  const [at, setAt] = createSignal({ top: 0, left: 0 });
  const place = () => {
    const row = props.anchor.getBoundingClientRect();
    const list = props.anchor.closest("nav")?.getBoundingClientRect() ?? row;
    const height = note?.offsetHeight ?? 0;
    setAt({
      top: Math.max(8, Math.min(row.top, window.innerHeight - height - 8)),
      left: list.right + 8,
    });
  };
  onMount(() => {
    place();
    const away = (event: PointerEvent) => {
      const target = event.target as Element;
      // The ⓘ toggles it itself.
      if (note?.contains(target) || target.closest("[data-note]")) return;
      props.onClose();
    };
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    document.addEventListener("pointerdown", away, true);
    onCleanup(() => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", away, true);
    });
  });
  useFirstKeys({ Escape: () => props.onClose() });
  const judge = () =>
    [importance(props.file), props.file.judge_reason]
      .filter(Boolean)
      .join(": ");
  return (
    <div
      ref={note}
      class={styles.note}
      role="dialog"
      aria-label="Why this file"
      style={{ top: `${at().top}px`, left: `${at().left}px` }}
      // Solid hands a portal's clicks on to the row it opened from.
      onClick={(event) => event.stopPropagation()}
    >
      <Show when={props.file.reason}>
        {(reason) => <Part heading="Guide" text={reason()} />}
      </Show>
      <Show when={judge()}>
        {(text) => <Part heading="Judge" text={text()} />}
      </Show>
      <Show when={props.file.why}>
        {(why) => <Part heading={ruled(props.file)} text={why()} />}
      </Show>
      {/* A judged file is placed by its score, not the heuristics. */}
      <Show when={props.file.importance === null && props.file.hint}>
        {(hint) => <Part heading="Heuristics" text={hint()} />}
      </Show>
      <p class={styles.path}>
        <bdi dir="ltr">{props.file.path}</bdi>
      </p>
      <p class={styles.keys}>i toggles · esc closes</p>
    </div>
  );
}

/** What the rule did, or the guard for a file no rule matched. */
function ruled(file: File): string {
  if (!file.rule) return "Kept in review";
  if (file.promoted) return `Promoted by the judge from ${file.rule}`;
  return `${file.tier === "trusted" ? "Trusted" : "Kept in review"} by ${file.rule}`;
}

function Part(props: { heading: string; text: string }) {
  return (
    <section class={styles.part}>
      <h3>{props.heading}</h3>
      <p>
        <CodeText text={props.text} />
      </p>
    </section>
  );
}
