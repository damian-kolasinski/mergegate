import { batch, createSignal, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { EditDraft } from "../../api/EditDraft";
import type { ItemDiff } from "../../api/ItemDiff";
import type { Review } from "../../api/Review";
import type { WriteDraft } from "../../api/WriteDraft";
import { deleteJson, postJson, putJson } from "../../app/hub";
import type { Span, Spot } from "../../diff-view/diff-view";
import { Button } from "../../design-system";
import { sideOf, spotOf } from "./anchor";
import type { Comments } from "./comments";
import type { CommentEditor } from "./editor";
import styles from "./drafts.module.css";

export type Drafts = ReturnType<typeof useDrafts>;

/**
 * The operator's draft comments on every revision, which stay on the hub
 * until a review sends them all.
 */
export function useDrafts(
  diff: ItemDiff,
  options: {
    editor: CommentEditor;
    reviewable: () => boolean;
    reveal: (spot: Spot) => void;
    isSent: (comment: Comment) => boolean;
  },
) {
  const { editor } = options;
  const url = `/api/items/${encodeURIComponent(diff.item.id)}/drafts`;
  const [list, setList] = createSignal(diff.drafts);
  // The span the new comment is on, while its editor is open.
  const [span, setSpan] = createSignal<Span>();
  const [deleting, setDeleting] = createSignal<string>();
  const [failure, setFailure] = createSignal<{ id: string; message: string }>();
  editor.onReset(() => setFailure(undefined));
  const isDraft = (comment: Comment) =>
    list().some((other) => other.id === comment.id);
  const editKey = (comment: Comment) => `edit ${comment.id}`;
  /** Drafts that went out, from this tab or another. */
  const sent = (comments: Comment[]) => {
    const ids = new Set(comments.map((comment) => comment.id));
    setList((all) => all.filter((comment) => !ids.has(comment.id)));
    const failed = failure();
    if (failed && ids.has(failed.id)) setFailure(undefined);
  };

  return {
    list,
    isDraft,
    /** Drafts change only while the hub takes a review. */
    canChange: (comment: Comment) => options.reviewable() && isDraft(comment),
    deleting,
    /** Why deleting `comment` failed. */
    failure: (comment: Comment) => {
      const failed = failure();
      return failed?.id === comment.id ? failed.message : undefined;
    },
    /** Where the new comment is being written. */
    composing: () => (editor.isOpen("new") ? span() : undefined),
    editing: (comment: Comment) => editor.isOpen(editKey(comment)),
    start(on: Span) {
      // Together, so no update sees the new editor on the old span.
      batch(() => {
        const opened = editor.start({
          key: "new",
          show: () => options.reveal(on),
          save: (text) => {
            const body: WriteDraft = {
              revision: diff.item.revision.number,
              path: on.path,
              side: sideOf[on.side],
              start_line: on.start,
              line: on.line,
              body: text,
            };
            return postJson<Comment>(url, body).then((comment) => {
              // A review that went out meanwhile may have sent it already.
              if (!options.isSent(comment)) setList((all) => [...all, comment]);
            });
          },
        });
        if (opened) setSpan(on);
      });
    },
    edit(comment: Comment) {
      editor.start({
        key: editKey(comment),
        text: comment.body,
        show: () => options.reveal(spotOf(comment)),
        save: (text) => {
          const body: EditDraft = { body: text };
          return putJson<Comment>(`${url}/${comment.id}`, body).then((edited) =>
            setList((all) =>
              all.map((other) => (other.id === edited.id ? edited : other)),
            ),
          );
        },
      });
    },
    remove(comment: Comment) {
      setDeleting(comment.id);
      setFailure(undefined);
      deleteJson(`${url}/${comment.id}`)
        .then(
          () =>
            setList((all) => all.filter((other) => other.id !== comment.id)),
          (error: unknown) =>
            setFailure({
              id: comment.id,
              message: error instanceof Error ? error.message : String(error),
            }),
        )
        .finally(() => setDeleting(undefined));
    },
    sent,
    /** A review sent every draft the hub had for the item. */
    submitted(review: Review) {
      sent(review.comments);
      // The review waited for every editor to be empty.
      for (const comment of review.comments) editor.close(editKey(comment));
    },
  };
}

/** Edit and Delete on a draft the hub still takes changes to. */
export function DraftActions(props: { comments: Comments; comment: Comment }) {
  return (
    <Show when={props.comments.drafts.canChange(props.comment)}>
      <span class={styles.actions}>
        <Button onClick={() => props.comments.drafts.edit(props.comment)}>
          Edit
        </Button>
        <Button
          disabled={props.comments.drafts.deleting() !== undefined}
          onClick={() => props.comments.drafts.remove(props.comment)}
        >
          Delete
        </Button>
      </span>
    </Show>
  );
}
