import { batch, createSignal, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { EditDraft } from "../../api/EditDraft";
import type { ItemDiff } from "../../api/ItemDiff";
import type { MoveDraft } from "../../api/MoveDraft";
import type { Review } from "../../api/Review";
import type { WriteDraft } from "../../api/WriteDraft";
import { deleteJson, postJson, putJson } from "../../app/hub";
import type { Span, Spot } from "../../diff-view/diff-view";
import { Button } from "../../design-system";
import { isFlagged, sideOf } from "./anchor";
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
    /** Closes the Conversation list, so the diff's lines can be picked. */
    hideAll: () => void;
    /** Puts the comment `id` back on the page. */
    show: (id: string) => void;
    isSent: (comment: Comment) => boolean;
  },
) {
  const { editor } = options;
  const url = `/api/items/${encodeURIComponent(diff.item.id)}/drafts`;
  const [list, setList] = createSignal(diff.drafts);
  // The span the new comment is on, while its editor is open.
  const [span, setSpan] = createSignal<Span>();
  const [deleting, setDeleting] = createSignal<string>();
  // The flagged draft whose new lines the operator is picking.
  const [placing, setPlacing] = createSignal<Comment>();
  const revision = diff.item.revision.number;
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
    if (ids.has(placing()?.id ?? "")) setPlacing(undefined);
  };
  const failed = (comment: Comment) => (error: unknown) =>
    setFailure({
      id: comment.id,
      message: error instanceof Error ? error.message : String(error),
    });
  /** Puts `comment` on `span` of the revision on screen, keeping its body. */
  const move = (comment: Comment, span: Span) => {
    const body: MoveDraft = {
      revision,
      path: span.path,
      side: sideOf[span.side],
      start_line: span.start,
      line: span.line,
    };
    setFailure(undefined);
    return putJson<Comment>(`${url}/${comment.id}/anchor`, body).then(
      (moved) => {
        setList((all) =>
          all.map((other) => (other.id === moved.id ? moved : other)),
        );
        options.reveal(span);
      },
      failed(comment),
    );
  };

  return {
    list,
    isDraft,
    /** Drafts change only while the hub takes a review. */
    canChange: (comment: Comment) => options.reviewable() && isDraft(comment),
    /** Earlier revisions' drafts a review on this one can't take along. */
    flagged: () => list().filter((draft) => isFlagged(draft, revision)),
    isFlagged: (comment: Comment) =>
      isDraft(comment) && isFlagged(comment, revision),
    placing,
    /** Starts picking new lines for a flagged draft. */
    replace(comment: Comment) {
      setFailure(undefined);
      setPlacing(comment);
      options.hideAll();
    },
    /** Puts the draft being re-placed on the lines picked. */
    place(span: Span) {
      const comment = placing();
      if (!comment) return;
      setPlacing(undefined);
      move(comment, span);
    },
    stopPlacing: () => setPlacing(undefined),
    /** Keeps a draft on the lines that replaced its own. */
    keep(comment: Comment) {
      const placement = comment.placement;
      if (comment.path === null || placement?.kind !== "changed") return;
      move(comment, {
        path: comment.path,
        side: comment.side === "old" ? "deletions" : "additions",
        start: placement.start_line,
        line: placement.line,
      });
    },
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
        comment: comment.id,
        text: comment.body,
        show: () => options.show(comment.id),
        save: (text) => {
          const body: EditDraft = { body: text };
          return putJson<Comment>(`${url}/${comment.id}`, body).then((edited) =>
            setList((all) =>
              all.map((other) =>
                // The hub places drafts on reads of the conversation only,
                // and an edit leaves the lines where they were.
                other.id === edited.id
                  ? { ...other, body: edited.body }
                  : other,
              ),
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
          failed(comment),
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

/**
 * Edit and Delete on a draft the hub still takes changes to, and, on one a
 * review can't take along, Re-place, and Keep here when its lines changed.
 */
export function DraftActions(props: { comments: Comments; comment: Comment }) {
  const drafts = () => props.comments.drafts;
  const flagged = () => drafts().isFlagged(props.comment);
  return (
    <Show when={drafts().canChange(props.comment)}>
      <span class={styles.actions}>
        <Show when={flagged() && props.comment.placement?.kind === "changed"}>
          <Button onClick={() => drafts().keep(props.comment)}>
            Keep here
          </Button>
        </Show>
        <Show when={flagged()}>
          <Button onClick={() => drafts().replace(props.comment)}>
            Re-place
          </Button>
        </Show>
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
