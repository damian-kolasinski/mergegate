import { createEffect, createSignal, on, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import { Button } from "../../design-system";
import styles from "./collapse.module.css";
import type { Comments } from "./comments";
import type { CommentEditor } from "./editor";

export type Collapse = ReturnType<typeof useCollapse>;

/**
 * The threads the operator folded to a line, as GitHub folds a
 * conversation. Kept per item in the tab's session, so a reload or the
 * switch to a newer revision, which builds the review anew, keeps them
 * folded.
 */
export function useCollapse(item: string, editor: CommentEditor) {
  const [folded, setFolded] = createSignal<ReadonlySet<string>>(read(item));
  createEffect(() => write(item, folded()));
  const unfold = (id: string) => {
    if (!folded().has(id)) return;
    setFolded((ids) => {
      const rest = new Set(ids);
      rest.delete(id);
      return rest;
    });
  };
  // An editor opening on a folded thread, a reply or an override, would be
  // out of sight with what's typed into it, so the thread unfolds, and stays
  // so once it closes.
  createEffect(
    on(
      () => editor.open()?.comment,
      (id) => id && unfold(id),
    ),
  );
  // Nor does a thread with the editor open on it fold: the editor would go
  // with it but keep the keys and the review held.
  const writing = (id: string) => editor.open()?.comment === id;
  return {
    folded: (comment: Comment) => folded().has(comment.id),
    writing: (comment: Comment) => writing(comment.id),
    toggle(comment: Comment) {
      if (folded().has(comment.id)) unfold(comment.id);
      else if (!writing(comment.id)) {
        setFolded((ids) => new Set(ids).add(comment.id));
      }
    },
    /** Folds or unfolds every one of `comments`. */
    all(comments: readonly Comment[], fold: boolean) {
      const ids = fold ? comments.map((comment) => comment.id) : [];
      setFolded(new Set(ids.filter((id) => !writing(id))));
    },
  };
}

/** The chevron that folds a thread, first on its card's header. */
export function FoldButton(props: { comments: Comments; comment: Comment }) {
  const folded = () => props.comments.collapse.folded(props.comment);
  const writing = () => props.comments.collapse.writing(props.comment);
  return (
    <button
      type="button"
      class={styles.fold}
      disabled={writing()}
      aria-expanded={!folded()}
      aria-label={`${folded() ? "Expand" : "Collapse"} comment ${props.comment.id}`}
      title={
        writing()
          ? "Save or cancel what's open on it first"
          : folded()
            ? "Expand"
            : "Collapse"
      }
      onClick={(event) => {
        props.comments.collapse.toggle(props.comment);
        // A focused button would take Enter from the shortcuts.
        event.currentTarget.blur();
      }}
    >
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path d="M6 4l4 4-4 4" />
      </svg>
    </button>
  );
}

/**
 * The header's button that folds every thread, or unfolds them once all
 * are: GitHub's "Hide comments" for the whole item.
 */
export function CollapseAll(props: { comments: Comments }) {
  const all = () => props.comments.threads.all();
  const folded = () =>
    all().every((comment) => props.comments.collapse.folded(comment));
  return (
    <Show when={all().length}>
      <Button keyHint="z" onClick={() => toggleAll(props.comments)}>
        {folded() ? "Expand comments" : "Collapse comments"}
      </Button>
    </Show>
  );
}

/** `z`: what the header's button does. */
export function toggleAll(comments: Comments) {
  const all = comments.threads.all();
  comments.collapse.all(
    all,
    !all.every((comment) => comments.collapse.folded(comment)),
  );
}

function key(item: string): string {
  return `mergegate:folded:${item}`;
}

// The session may be unavailable, as in a private window: then threads stay
// folded only as long as the screen.
function read(item: string): ReadonlySet<string> {
  try {
    const kept: unknown = JSON.parse(sessionStorage.getItem(key(item)) ?? "[]");
    if (Array.isArray(kept)) {
      return new Set(kept.filter((id) => typeof id === "string"));
    }
  } catch {
    // Unreadable is as good as none.
  }
  return new Set();
}

function write(item: string, folded: ReadonlySet<string>): void {
  try {
    if (folded.size) {
      sessionStorage.setItem(key(item), JSON.stringify([...folded]));
    } else {
      sessionStorage.removeItem(key(item));
    }
  } catch {
    // See `read`.
  }
}
