import { createSignal } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import { kept, merged } from "./merge";

export type Sent = ReturnType<typeof useSent>;

/** The comments reviews have sent to the agent, on every revision. */
export function useSent(diff: ItemDiff) {
  const [list, setList] = createSignal(diff.comments);
  return {
    list,
    has: (comment: Comment) => list().some((other) => other.id === comment.id),
    /** Sent by a review from this tab. */
    add(comments: Comment[]) {
      setList((all) => merged(all, comments));
    },
    /** The hub's list, which another tab's reviews may have added to. */
    keep(latest: Comment[]) {
      setList((all) => kept(all, latest));
    },
  };
}
