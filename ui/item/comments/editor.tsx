import { createSignal, onMount, Show } from "solid-js";
import { Button, Meta, TextArea } from "../../design-system";
import styles from "./editor.module.css";

/** An editor open somewhere on the page. */
export type Open = {
  /** Which one it is: each place that holds an editor has its own key. */
  key: string;
  /** The comment it's on, unless it writes a new one. */
  comment?: string;
  text: string;
  /** Writes the text to the hub; the editor closes once it has. */
  save: (text: string) => Promise<unknown>;
  /** Puts the editor back on the page, where CodeView renders only what's near. */
  show: () => void;
};

/** The one comment being written, which nothing but its own buttons drops. */
export type CommentEditor = ReturnType<typeof useEditor>;

export function useEditor() {
  const [open, setOpen] = createSignal<Open>();
  // Locked while it saves, so what comes back is its.
  const [saving, setSaving] = createSignal(false);
  const [failure, setFailure] = createSignal<string>();
  // Each slice's own alerts, which go away with the editor's, as they did
  // when every failure shared one slot.
  const resets: (() => void)[] = [];
  const reset = () => {
    setFailure(undefined);
    for (const clear of resets) clear();
  };
  let field: HTMLTextAreaElement | undefined;

  const dirty = () => Boolean(open()?.text.trim());
  const focus = () => {
    if (field?.isConnected) focusOnceShown(field);
    // CodeView dropped the note with its file, or the list was closed;
    // shown again, the editor focuses.
    else open()?.show();
  };
  const cancel = () => {
    if (saving()) return;
    reset();
    setOpen(undefined);
  };

  return {
    open,
    /** Whether it holds text. */
    dirty,
    isOpen: (key: string) => open()?.key === key,
    saving,
    failure,
    /** Whether the open editor keeps a review from going out. */
    holding: () => dirty() || saving(),
    /** Opens `next`, unless that would drop what's typed in the open one. */
    start(next: Omit<Open, "text"> & { text?: string }): boolean {
      if (dirty()) {
        focus();
        return false;
      }
      reset();
      setOpen({ ...next, text: next.text ?? "" });
      return true;
    },
    /** Runs `clear` whenever an editor opens or is cancelled. */
    onReset(clear: () => void) {
      resets.push(clear);
    },
    /** Closes the editor keyed `key`, whose subject is gone. */
    close(key: string) {
      if (open()?.key === key) setOpen(undefined);
    },
    field(element: HTMLTextAreaElement) {
      field = element;
    },
    type(text: string) {
      setOpen((open) => open && { ...open, text });
    },
    cancel,
    /** Escape outside the editor: an empty one closes, one with text takes focus. */
    escape() {
      if (dirty()) focus();
      else cancel();
    },
    save() {
      const editing = open();
      if (!editing?.text.trim() || saving()) return;
      setSaving(true);
      setFailure(undefined);
      editing
        .save(editing.text)
        .then(
          () => setOpen(undefined),
          (error: unknown) =>
            setFailure(error instanceof Error ? error.message : String(error)),
        )
        .finally(() => setSaving(false));
    },
  };
}

/** The open editor, where `label` says it is. */
export function Editor(props: {
  editor: CommentEditor;
  label: string;
  /** What's written: a comment unless said otherwise. */
  noun?: string;
}) {
  let field: HTMLTextAreaElement | undefined;
  onMount(() => field && focusOnceShown(field));
  const empty = () => !props.editor.open()?.text.trim();
  return (
    <div
      class={styles.editor}
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          props.editor.save();
        } else if (event.key === "Escape" && empty()) {
          props.editor.cancel();
        }
      }}
    >
      <TextArea
        label={props.label}
        placeholder={`${props.noun ?? "Comment"} for the agent, sent with the review (⌘↩ saves)`}
        rows={3}
        value={props.editor.open()?.text ?? ""}
        disabled={props.editor.saving()}
        onInput={(text) => props.editor.type(text)}
        ref={(element) => {
          field = element;
          props.editor.field(element);
        }}
      />
      <div class={styles.actions}>
        <Show when={props.editor.failure()}>
          {(message) => (
            <span role="alert">
              <Meta tone="removed">{message()}</Meta>
            </span>
          )}
        </Show>
        <Button
          disabled={props.editor.saving()}
          onClick={() => props.editor.cancel()}
        >
          Cancel
        </Button>
        <Button
          variant="solid"
          disabled={empty() || props.editor.saving()}
          onClick={() => props.editor.save()}
        >
          Save draft
        </Button>
      </div>
    </div>
  );
}

/**
 * CodeView puts a note on the page on its own schedule, after the note is
 * rendered, so focus waits for it a few frames.
 */
function focusOnceShown(element: HTMLElement, frames = 30) {
  if (element.isConnected) {
    element.focus({ preventScroll: true });
    element.scrollIntoView({ block: "nearest" });
  } else if (frames > 0) {
    requestAnimationFrame(() => focusOnceShown(element, frames - 1));
  }
}
