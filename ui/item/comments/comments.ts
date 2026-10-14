import type { ItemDiff } from "../../api/ItemDiff";
import type { Comment } from "../../api/Comment";
import type { Review } from "../../api/Review";
import type { Spot } from "../../diff-view/diff-view";
import { spotOf } from "./anchor";
import { useDrafts } from "./drafts";
import { useEditor } from "./editor";
import { useReplies } from "./replies";
import { useReplyDrafts } from "./reply-drafts";
import { useSent } from "./sent";
import { useSync } from "./sync";
import { useThreads } from "./thread";

/**
 * The operator's line comments on an item, the one being written, and the
 * replies on them, the agent's and the operator's.
 */
export type Comments = ReturnType<typeof useComments>;

export function useComments(
  diff: ItemDiff,
  options: {
    reviewable: () => boolean;
    /** Scrolls a line into view, as CodeView renders only what's near. */
    reveal: (spot: Spot) => void;
    /** Opens the Conversation list, which holds the comments the diff doesn't. */
    showAll: () => void;
  },
) {
  const revision = diff.item.revision.number;
  const editor = useEditor();
  const sent = useSent(diff);
  /** Puts a comment back on the page: in the diff, or in the list. */
  const show = (id: string) => {
    const comment = threads.all().find((comment) => comment.id === id);
    if (!comment) return;
    if (comment.revision === revision) options.reveal(spotOf(comment));
    else options.showAll();
  };
  const drafts = useDrafts(diff, {
    ...options,
    editor,
    show,
    isSent: sent.has,
  });
  const replies = useReplies(diff);
  const replyDrafts = useReplyDrafts(diff, {
    editor,
    reviewable: options.reviewable,
    show,
    isSent: sent.has,
    isAnswered: replies.has,
  });
  const refresh = useSync(diff.item.id, { sent, drafts, replies, replyDrafts });
  const threads = useThreads(revision, {
    sent: sent.list,
    drafts: drafts.list,
    composing: drafts.composing,
  });
  return {
    /** The revision the diff shows. */
    revision,
    editor,
    sent,
    drafts,
    replies,
    replyDrafts,
    threads,
    /** The Conversation list mustn't close over text typed in it. */
    aside: () => {
      const id = editor.open()?.comment;
      const comment = threads.all().find((comment) => comment.id === id);
      return (
        editor.dirty() && comment !== undefined && comment.revision !== revision
      );
    },
    /** Scrolls to a comment the diff shows. */
    show(comment: Comment) {
      options.reveal(spotOf(comment));
    },
    /** The comment's thread, oldest first, the operator's drafts included. */
    thread: (comment: Comment) =>
      [...replies.to(comment), ...replyDrafts.to(comment)].sort(
        (a, b) => a.id - b.id,
      ),
    /** A review sent every draft the hub had for the item. */
    submitted(review: Review) {
      drafts.submitted(review);
      sent.add(review.comments);
      replyDrafts.submitted(review);
      replies.add(review.replies);
      // A refetch in flight may predate the review, and its answer would
      // drop what the review just sent.
      refresh();
    },
  };
}
