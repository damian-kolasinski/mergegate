import { createMemo, createSignal, For, onMount, Show } from "solid-js";
import type { Comment } from "../api/Comment";
import type { EditDraft } from "../api/EditDraft";
import type { ItemDiff } from "../api/ItemDiff";
import type { Review } from "../api/Review";
import type { Side } from "../api/Side";
import type { WriteDraft } from "../api/WriteDraft";
import { deleteJson, postJson, putJson } from "../app/hub";
import type { Span, Spot } from "../diff-view/diff-view";
import { Badge, Button, Meta, TextArea } from "../design-system";
import styles from "./comments.module.css";

type Editor =
  | { kind: "new"; span: Span; text: string }
  | { kind: "edit"; id: string; text: string };

/** The operator's line comments on an item, and the one being written. */
export type Comments = ReturnType<typeof useComments>;

/**
 * Drafts stay on the hub until a review sends them all. The diff shows the
 * shown revision's drafts and sent comments under their last line.
 */
export function useComments(diff: ItemDiff) {
  const revision = diff.item.revision.number;
  const drafts = `/api/items/${encodeURIComponent(diff.item.id)}/drafts`;
  const [draft, setDraft] = createSignal(diff.drafts);
  const [sent, setSent] = createSignal(diff.comments);
  const [editor, setEditor] = createSignal<Editor>();
  const [saving, setSaving] = createSignal(false);
  const [failure, setFailure] = createSignal<string>();

  const shown = createMemo(() =>
    [...sent(), ...draft()].filter((comment) => comment.revision === revision),
  );
  const spots = createMemo(() => {
    const spots = shown().map(spotOf);
    const open = editor();
    if (open?.kind === "new") spots.push(open.span);
    return spots;
  });
  const isDraft = (comment: Comment) =>
    draft().some((other) => other.id === comment.id);

  const run = (request: Promise<unknown>) => {
    setSaving(true);
    setFailure(undefined);
    request
      .then(
        () => setEditor(undefined),
        (error: unknown) =>
          setFailure(error instanceof Error ? error.message : String(error)),
      )
      .finally(() => setSaving(false));
  };

  return {
    /** On every revision; the next review sends them all. */
    drafts: draft,
    spots,
    editor,
    saving,
    failure,
    isDraft,
    at: (spot: Spot) =>
      shown().filter((comment) => sameSpot(spotOf(comment), spot)),
    start(span: Span) {
      setFailure(undefined);
      setEditor({ kind: "new", span, text: "" });
    },
    edit(comment: Comment) {
      setFailure(undefined);
      setEditor({ kind: "edit", id: comment.id, text: comment.body });
    },
    type(text: string) {
      setEditor((open) => open && { ...open, text });
    },
    cancel() {
      setFailure(undefined);
      setEditor(undefined);
    },
    save() {
      const open = editor();
      if (!open?.text.trim() || saving()) return;
      if (open.kind === "new") {
        const body: WriteDraft = {
          revision,
          path: open.span.path,
          side: sideOf[open.span.side],
          start_line: open.span.start,
          line: open.span.line,
          body: open.text,
        };
        run(
          postJson<Comment>(drafts, body).then((comment) =>
            setDraft((all) => [...all, comment]),
          ),
        );
      } else {
        const body: EditDraft = { body: open.text };
        run(
          putJson<Comment>(`${drafts}/${open.id}`, body).then((comment) =>
            setDraft((all) =>
              all.map((other) => (other.id === comment.id ? comment : other)),
            ),
          ),
        );
      }
    },
    remove(comment: Comment) {
      run(
        deleteJson(`${drafts}/${comment.id}`).then(() =>
          setDraft((all) => all.filter((other) => other.id !== comment.id)),
        ),
      );
    },
    /** A review sent every draft on the item. */
    submitted(review: Review) {
      setDraft([]);
      setSent((all) => [...all, ...review.comments]);
    },
  };
}

/** What's under one line of the diff: its comments, then a new one. */
export function Thread(props: { comments: Comments; spot: Spot }) {
  const editing = (comment: Comment) => {
    const open = props.comments.editor();
    return open?.kind === "edit" && open.id === comment.id;
  };
  const composing = () => {
    const open = props.comments.editor();
    return open?.kind === "new" && sameSpot(open.span, props.spot)
      ? open.span
      : undefined;
  };
  return (
    <div class={styles.thread}>
      <For each={props.comments.at(props.spot)}>
        {(comment) => (
          <Show
            when={!editing(comment)}
            fallback={
              <Editor
                comments={props.comments}
                label={`Edit comment ${comment.id}`}
              />
            }
          >
            <article
              class={styles.comment}
              aria-label={`Comment ${comment.id}`}
            >
              <header class={styles.meta}>
                <Show
                  when={props.comments.isDraft(comment)}
                  fallback={<Badge tone="success">Sent</Badge>}
                >
                  <Badge tone="warning">Draft</Badge>
                </Show>
                <Meta>{linesOf(comment.start_line, comment.line)}</Meta>
                <Show when={props.comments.isDraft(comment)}>
                  <span class={styles.actions}>
                    <Button onClick={() => props.comments.edit(comment)}>
                      Edit
                    </Button>
                    <Button
                      disabled={props.comments.saving()}
                      onClick={() => props.comments.remove(comment)}
                    >
                      Delete
                    </Button>
                  </span>
                </Show>
              </header>
              <p class={styles.body}>{comment.body}</p>
            </article>
          </Show>
        )}
      </For>
      <Show when={composing()}>
        {(span) => (
          <Editor
            comments={props.comments}
            label={`Comment on ${linesOf(span().start, span().line).toLowerCase()}`}
          />
        )}
      </Show>
    </div>
  );
}

function Editor(props: { comments: Comments; label: string }) {
  let field: HTMLTextAreaElement | undefined;
  onMount(() => field && focusOnceShown(field));
  const empty = () => !props.comments.editor()?.text.trim();
  return (
    <div
      class={styles.editor}
      onKeyDown={(event) => {
        if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
          event.preventDefault();
          props.comments.save();
        } else if (event.key === "Escape" && empty()) {
          props.comments.cancel();
        }
      }}
    >
      <TextArea
        label={props.label}
        placeholder="Comment for the agent, sent with the review (⌘↩ saves)"
        rows={3}
        value={props.comments.editor()?.text ?? ""}
        disabled={props.comments.saving()}
        onInput={(text) => props.comments.type(text)}
        ref={(element) => {
          field = element;
        }}
      />
      <div class={styles.editorActions}>
        <Show when={props.comments.failure()}>
          {(message) => <Meta tone="removed">{message()}</Meta>}
        </Show>
        <Button onClick={() => props.comments.cancel()}>Cancel</Button>
        <Button
          variant="solid"
          disabled={empty() || props.comments.saving()}
          onClick={() => props.comments.save()}
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

const sideOf: Record<Spot["side"], Side> = {
  deletions: "old",
  additions: "new",
};

function spotOf(comment: Comment): Spot {
  return {
    path: comment.path,
    side: comment.side === "old" ? "deletions" : "additions",
    line: comment.line,
  };
}

function sameSpot(a: Spot, b: Spot): boolean {
  return a.path === b.path && a.side === b.side && a.line === b.line;
}

function linesOf(start: number, line: number): string {
  return start === line ? `Line ${line}` : `Lines ${start}–${line}`;
}
