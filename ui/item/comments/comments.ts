import type { ItemDiff } from "../../api/ItemDiff";
import type { Comment } from "../../api/Comment";
import type { Review } from "../../api/Review";
import type { Spot } from "../../diff-view/diff-view";
import { spotOf } from "./anchor";
import { useDrafts } from "./drafts";
import { useEditor } from "./editor";
import { useExternal } from "./external";
import { useReplies } from "./replies";
import { useReplyDrafts } from "./reply-drafts";
import { useSent } from "./sent";
import { useSync } from "./sync";
import { useThreads } from "./thread";

/**
 * The operator's line comments on an item, the one being written, those
 * from its pull request on GitHub, and the replies on them, the agent's and
 * the operator's.
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
    /** Closes the list, so the diff's lines can be picked. */
    hideAll: () => void;
    /** Lines of a side the diff on screen has, as an interdiff has no base. */
    onScreen: (spot: Spot) => boolean;
    /**
     * The revision the diff on screen compares against, null for all
     * changes, and its files unchanged since then. Comparing another way
     * keeps the screen, so these follow it.
     */
    since: () => number | null;
    unchanged: () => ReadonlySet<string>;
  },
) {
  const revision = diff.item.revision.number;
  const editor = useEditor();
  const sent = useSent(diff);
  /** Puts a comment back on the page: in the diff, or in the list. */
  const show = (id: string) => {
    const comment = threads.all().find((comment) => comment.id === id);
    if (!comment) return;
    const spot = threads.inDiff(comment) && spotOf(comment, revision);
    if (spot) options.reveal(spot);
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
  const refresh = useSync(diff.item.id, revision, {
    sent,
    drafts,
    replies,
    replyDrafts,
  });
  const external = useExternal({
    editor,
    reviewable: options.reviewable,
    show,
    sent,
    replies,
  });
  /**
   * Over a diff since rK, a thread the operator should see again: theirs
   * not sent yet, written on rK or later, or one the agent answered last,
   * unless the answer is a fix rK already had or a rejection they agreed to.
   */
  const waiting = (comment: Comment) => {
    const since = options.since();
    if (since === null) return false;
    if (drafts.isDraft(comment) || replyDrafts.to(comment).length) return true;
    if (comment.revision >= since) return true;
    const latest = replies.to(comment).at(-1);
    if (latest?.author !== "agent" || external.agreed(comment)) return false;
    return (
      latest.status !== "addressed" ||
      latest.addressed_in === null ||
      latest.addressed_in > since
    );
  };
  const threads = useThreads(
    revision,
    {
      sent: sent.list,
      drafts: drafts.list,
      composing: drafts.composing,
    },
    options.onScreen,
    { since: options.since, paths: options.unchanged, waiting },
  );
  return {
    /** The revision the diff shows. */
    revision,
    editor,
    sent,
    drafts,
    replies,
    replyDrafts,
    external,
    threads,
    /** The Conversation list mustn't close over text typed in it. */
    aside: () => {
      const id = editor.open()?.comment;
      const comment = threads.all().find((comment) => comment.id === id);
      return (
        editor.dirty() && comment !== undefined && !threads.inDiff(comment)
      );
    },
    /** The line the open editor is under in the diff, if it's there. */
    writing: (): Spot | undefined => {
      const composing = drafts.composing();
      if (composing) return composing;
      const id = editor.open()?.comment;
      const comment = threads.all().find((comment) => comment.id === id);
      return comment && threads.inDiff(comment)
        ? spotOf(comment, revision)
        : undefined;
    },
    /** Scrolls to a comment the diff shows. */
    show(comment: Comment) {
      const spot = spotOf(comment, revision);
      if (spot) options.reveal(spot);
    },
    /** The comment's thread, oldest first, the operator's drafts included. */
    thread: (comment: Comment) =>
      [...replies.to(comment), ...replyDrafts.to(comment)].sort(
        (a, b) => a.id - b.id,
      ),
    /**
     * The hub held a review over drafts that can't move, which another tab
     * may have written: they're fetched so they can be re-placed or deleted.
     */
    held: () => refresh(true),
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
