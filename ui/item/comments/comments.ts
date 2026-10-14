import type { ItemDiff } from "../../api/ItemDiff";
import type { Review } from "../../api/Review";
import type { Spot } from "../../diff-view/diff-view";
import { useDrafts } from "./drafts";
import { useEditor } from "./editor";
import { useReplies } from "./replies";
import { useSent } from "./sent";
import { useSync } from "./sync";
import { useThreads } from "./thread";

/**
 * The operator's line comments on an item, the one being written, and the
 * agent's replies to them.
 */
export type Comments = ReturnType<typeof useComments>;

export function useComments(
  diff: ItemDiff,
  options: {
    reviewable: () => boolean;
    /** Scrolls a line into view, as CodeView renders only what's near. */
    reveal: (spot: Spot) => void;
  },
) {
  const editor = useEditor();
  const sent = useSent(diff);
  const drafts = useDrafts(diff, { ...options, editor, isSent: sent.has });
  const replies = useReplies(diff);
  useSync(diff.item.id, { sent, drafts, replies });
  const threads = useThreads(diff.item.revision.number, {
    sent: sent.list,
    drafts: drafts.list,
    composing: drafts.composing,
  });
  return {
    editor,
    sent,
    drafts,
    replies,
    threads,
    /** A review sent every draft the hub had for the item. */
    submitted(review: Review) {
      drafts.submitted(review);
      sent.add(review.comments);
    },
  };
}
