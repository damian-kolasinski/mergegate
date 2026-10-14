import { createSignal, For, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import type { Reply } from "../../api/Reply";
import type { ReplyStatus } from "../../api/ReplyStatus";
import { Badge, Meta, type Tone } from "../../design-system";
import type { Comments } from "./comments";
import { kept } from "./merge";
import styles from "./replies.module.css";

export type Replies = ReturnType<typeof useReplies>;

/** The agent's replies to sent comments, which `mg reply` adds. */
export function useReplies(diff: ItemDiff) {
  const [list, setList] = createSignal(diff.replies);
  return {
    /** The comment's thread, oldest first. */
    to: (comment: Comment) =>
      list().filter((reply) => reply.comment === comment.id),
    keep(latest: Reply[]) {
      setList((all) => kept(all, latest));
    },
  };
}

/** A sent comment's thread: the agent's replies under it. */
export function ReplyList(props: { comments: Comments; comment: Comment }) {
  return (
    <Show when={props.comments.replies.to(props.comment).length}>
      <ol class={styles.replies} aria-label="Replies">
        <For each={props.comments.replies.to(props.comment)}>
          {(reply) => <ReplyEntry reply={reply} />}
        </For>
      </ol>
    </Show>
  );
}

function ReplyEntry(props: { reply: Reply }) {
  return (
    <li>
      <header class={styles.meta}>
        <Meta>{props.reply.author === "agent" ? "Agent" : "You"}</Meta>
        <Show when={props.reply.status}>
          {(status) => (
            <StatusBadge
              status={status()}
              addressedIn={props.reply.addressed_in}
            />
          )}
        </Show>
      </header>
      <Show when={props.reply.body}>
        <p class={styles.body}>{props.reply.body}</p>
      </Show>
    </li>
  );
}

/** Where the agent left a comment: addressed, rejected or a question. */
function StatusBadge(props: {
  status: ReplyStatus;
  addressedIn: number | null;
}) {
  return (
    <Badge tone={statusTone[props.status]}>
      {props.status === "addressed"
        ? addressedIn(props.addressedIn)
        : statusLabel[props.status]}
    </Badge>
  );
}

const statusLabel: Record<ReplyStatus, string> = {
  addressed: "Addressed",
  rejected: "Rejected",
  question: "Question",
};

const statusTone: Record<ReplyStatus, Tone> = {
  addressed: "success",
  rejected: "error",
  question: "brand",
};

/** Before the agent submits it, the change is only promised. */
function addressedIn(revision: number | null): string {
  return revision
    ? `Addressed in r${revision}`
    : "Addressed in the next revision";
}
