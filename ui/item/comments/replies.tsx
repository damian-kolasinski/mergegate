import { createSignal, For, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import type { Reply } from "../../api/Reply";
import type { ReplyStatus } from "../../api/ReplyStatus";
import { Badge, Meta, type Tone } from "../../design-system";
import type { Comments } from "./comments";
import { Editor } from "./editor";
import { kept, merged } from "./merge";
import { DraftReplyActions } from "./reply-drafts";
import styles from "./replies.module.css";

export type Replies = ReturnType<typeof useReplies>;

/**
 * The sent replies on sent comments: the agent's, which `mg reply` adds, and
 * the operator's, which went out with a review.
 */
export function useReplies(diff: ItemDiff) {
  const [list, setList] = createSignal(diff.replies);
  return {
    /** The comment's sent replies, oldest first. */
    to: (comment: Comment) =>
      list().filter((reply) => reply.comment === comment.id),
    has: (reply: Reply) => list().some((other) => other.id === reply.id),
    /** Sent by a review from this tab. */
    add(replies: Reply[]) {
      setList((all) => merged(all, replies));
    },
    keep(latest: Reply[]) {
      setList((all) => kept(all, latest));
    },
  };
}

/** A sent comment's thread, the operator's drafts included, and a reply. */
export function ReplyList(props: { comments: Comments; comment: Comment }) {
  return (
    <>
      <Show when={props.comments.thread(props.comment).length}>
        <ol class={styles.replies} aria-label="Replies">
          <For each={props.comments.thread(props.comment)}>
            {(reply) => <ReplyEntry comments={props.comments} reply={reply} />}
          </For>
        </ol>
      </Show>
      <Show when={props.comments.replyDrafts.replying(props.comment)}>
        <Editor
          editor={props.comments.editor}
          label={`Reply to ${props.comment.id}`}
          noun="Reply"
        />
      </Show>
    </>
  );
}

/** A reply, or its editor while a draft one is being edited. */
function ReplyEntry(props: { comments: Comments; reply: Reply }) {
  return (
    <li>
      <Show
        when={!props.comments.replyDrafts.editing(props.reply)}
        fallback={
          <Editor
            editor={props.comments.editor}
            label={`Edit reply to ${props.reply.comment}`}
            noun="Reply"
          />
        }
      >
        <header class={styles.meta}>
          <Meta>{props.reply.author === "agent" ? "Agent" : "You"}</Meta>
          <Show when={props.comments.replyDrafts.isDraft(props.reply)}>
            <Badge tone="warning">Draft</Badge>
            <DraftReplyActions comments={props.comments} reply={props.reply} />
          </Show>
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
        <Show when={props.comments.replyDrafts.failure(props.reply)}>
          {(message) => (
            <p role="alert">
              <Meta tone="removed">{message()}</Meta>
            </p>
          )}
        </Show>
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
