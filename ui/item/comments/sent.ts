import { createSignal } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import type { Review } from "../../api/Review";

/** The comments reviews have sent to the agent, on every revision. */
export function useSent(diff: ItemDiff) {
  const [list, setList] = createSignal(diff.comments);
  return {
    list,
    has: (comment: Comment) => list().some((other) => other.id === comment.id),
    submitted(review: Review) {
      setList((all) => [...all, ...review.comments]);
    },
  };
}
