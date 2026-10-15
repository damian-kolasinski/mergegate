import { createSignal } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import { kept, merged } from "./merge";

export type Sent = ReturnType<typeof useSent>;

/**
 * The comments reviews have sent to the agent, and those from GitHub, on
 * every revision.
 */
export function useSent(diff: ItemDiff) {
  const [list, setList] = createSignal(diff.comments);
  return {
    list,
    has: (comment: Comment) => list().some((other) => other.id === comment.id),
    /** Sent by a review from this tab. */
    add(comments: Comment[]) {
      setList((all) => merged(all, comments));
    },
    /**
     * One the hub changed, as agreeing with a rejection does. Its answer
     * isn't placed, and the lines the comment is on haven't changed.
     */
    update(comment: Comment) {
      setList((all) =>
        all.map((other) => {
          if (other.id !== comment.id) return other;
          const placement = comment.placement ?? other.placement;
          return placement ? { ...comment, placement } : comment;
        }),
      );
    },
    /** The hub's list, which another tab's reviews may have added to. */
    keep(latest: Comment[]) {
      setList((all) => kept(all, latest));
    },
  };
}
