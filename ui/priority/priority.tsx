import { createSignal, For, Show } from "solid-js";
import type { Item } from "../api/Item";
import type { Prioritize } from "../api/Prioritize";
import type { Priority } from "../api/Priority";
import { type Failure, failureOf, postJson } from "../app/hub";
import { useModalKeys } from "../app/keys";
import {
  Badge,
  Button,
  ErrorNote,
  Meta,
  TextArea,
  type Tone,
} from "../design-system";
import styles from "./priority.module.css";

const CHOICES: readonly (Priority | null)[] = ["p0", "p1", "p2", "p3", null];

const tone: Record<Priority, Tone> = {
  p0: "error",
  p1: "warning",
  p2: "brand",
  p3: "neutral",
};

/**
 * An item's priority, its note on hover; nothing without one. A `p0` is
 * filled: it outranks everything else on the row.
 */
export function PriorityBadge(props: {
  priority: Priority | null;
  note: string | null;
  /** The key that changes it on this screen, which the hover names. */
  keyHint?: string | undefined;
}) {
  const title = () =>
    [props.note, props.keyHint && `${props.keyHint} to change`]
      .filter(Boolean)
      .join(" · ") || undefined;
  return (
    <Show when={props.priority}>
      {(priority) => (
        <span class={styles.badge} title={title()}>
          <Badge tone={tone[priority()]} solid={priority() === "p0"}>
            {priority()}
          </Badge>
        </span>
      )}
    </Show>
  );
}

/**
 * Sets an item's priority and note together, as `mg prioritize` does: a
 * blank note clears the old one. `0`–`3` pick a priority and `n` none,
 * `Enter` saves and `Escape` cancels, unless the note has the keys. The
 * screen behind hears no keys while it's open.
 */
export function PriorityPicker(props: {
  item: string;
  priority: Priority | null;
  note: string | null;
  onClose: () => void;
  onSaved: (item: Item) => void;
}) {
  // Seeded once: the picker opens on what the item has now.
  // eslint-disable-next-line solid/reactivity
  const [choice, setChoice] = createSignal(props.priority);
  // eslint-disable-next-line solid/reactivity
  const [note, setNote] = createSignal(props.note ?? "");
  const [sending, setSending] = createSignal(false);
  const [failure, setFailure] = createSignal<Failure>();

  const save = () => {
    if (sending()) return;
    const body: Prioritize = {
      priority: choice(),
      note: note().trim() || null,
    };
    setSending(true);
    setFailure(undefined);
    postJson<Item>(
      `/api/items/${encodeURIComponent(props.item)}/priority`,
      body,
    )
      .then(props.onSaved, (error: unknown) => setFailure(failureOf(error)))
      .finally(() => setSending(false));
  };

  let panel: HTMLDivElement | undefined;
  useModalKeys(
    {
      "0": () => setChoice("p0"),
      "1": () => setChoice("p1"),
      "2": () => setChoice("p2"),
      "3": () => setChoice("p3"),
      n: () => setChoice(null),
      // The keys take Enter from a focused button too, so Cancel still
      // cancels.
      Enter: () => {
        const focused = document.activeElement;
        if (focused instanceof HTMLButtonElement && panel?.contains(focused)) {
          focused.click();
        } else {
          save();
        }
      },
      Escape: () => props.onClose(),
    },
    {
      group: "Priority",
      keys: [
        ["0 – 3", "Pick p0 to p3"],
        ["n", "No priority"],
        ["Enter", "Save (⌘↩ from the note)"],
        ["Esc", "Cancel"],
      ],
    },
  );

  // The scrim keeps clicks off the screen behind, which has dialogs of its
  // own that would fight this one for the keys.
  return (
    <>
      <div class={styles.scrim} onClick={() => props.onClose()} />
      <div
        ref={panel}
        class={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={`Priority of ${props.item}`}
      >
        <Meta>Priority of {props.item}</Meta>
        <div class={styles.choices} role="radiogroup" aria-label="Priority">
          <For each={CHOICES}>
            {(value) => (
              <button
                type="button"
                role="radio"
                class={styles.choice}
                aria-checked={choice() === value}
                data-tone={value ? tone[value] : "none"}
                disabled={sending()}
                // Keeps focus where it was, so Enter saves after a click.
                tabIndex={-1}
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => setChoice(value)}
              >
                {value ?? "none"}
              </button>
            )}
          </For>
        </div>
        <TextArea
          label="Note"
          placeholder="Why (optional) — ⌘↩ saves"
          rows={2}
          value={note()}
          disabled={sending()}
          onInput={setNote}
          onSubmit={save}
        />
        <div class={styles.actions}>
          <Show when={failure()}>
            {(failure) => <ErrorNote failure={failure()} />}
          </Show>
          <Button disabled={sending()} onClick={() => props.onClose()}>
            Cancel
          </Button>
          <Button variant="solid" busy={sending()} keyHint="↵" onClick={save}>
            {sending() ? "Saving…" : "Save"}
          </Button>
        </div>
      </div>
    </>
  );
}
