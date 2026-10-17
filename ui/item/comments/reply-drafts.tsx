import { createSignal, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ItemDiff } from "../../api/ItemDiff";
import type { Reply } from "../../api/Reply";
import type { Review } from "../../api/Review";
import type { WriteDraftReply } from "../../api/WriteDraftReply";
import {
  deleteJson,
  type Failure,
  failureOf,
  postJson,
  putJson,
} from "../../app/hub";
import { Button } from "../../design-system";
import type { Comments } from "./comments";
import type { CommentEditor } from "./editor";
import styles from "./reply-drafts.module.css";

export type ReplyDrafts = ReturnType<typeof useReplyDrafts>;

/**
 * The operator's draft replies to sent comments, which stay on the hub until
 * a review sends them all.
 */
export function useReplyDrafts(
  diff: ItemDiff,
  options: {
    editor: CommentEditor;
    reviewable: () => boolean;
    /** Puts the comment `id` back on the page. */
    show: (id: string) => void;
    isSent: (comment: Comment) => boolean;
    isAnswered: (reply: Reply) => boolean;
  },
) {
  const { editor } = options;
  const [list, setList] = createSignal(diff.draft_replies);
  const [deleting, setDeleting] = createSignal<number>();
  const [failure, setFailure] = createSignal<{ id: number } & Failure>();
  editor.onReset(() => setFailure(undefined));
  const replyKey = (comment: string) => `reply to ${comment}`;
  const editKey = (reply: Reply) => `edit reply ${reply.id}`;
  /** Draft replies that went out, from this tab or another. */
  const sent = (replies: Reply[]) => {
    const ids = new Set(replies.map((reply) => reply.id));
    setList((all) => all.filter((reply) => !ids.has(reply.id)));
    const failed = failure();
    if (failed && ids.has(failed.id)) setFailure(undefined);
  };

  return {
    list,
    /** The comment's draft replies, oldest first. */
    to: (comment: Comment) =>
      list().filter((reply) => reply.comment === comment.id),
    isDraft: (reply: Reply) => list().some((other) => other.id === reply.id),
    /**
     * Sent comments take replies while the hub takes a review, except one
     * the operator kept from the agent, as no one would read the reply.
     */
    canReply: (comment: Comment) =>
      options.reviewable() &&
      options.isSent(comment) &&
      comment.external?.vote !== "down",
    /** Draft replies change only while the hub takes a review. */
    canChange: options.reviewable,
    deleting,
    /** Why deleting `reply` failed. */
    failure: (reply: Reply) => {
      const failed = failure();
      return failed?.id === reply.id ? failed : undefined;
    },
    replying: (comment: Comment) => editor.isOpen(replyKey(comment.id)),
    editing: (reply: Reply) => editor.isOpen(editKey(reply)),
    reply(comment: Comment) {
      editor.start({
        key: replyKey(comment.id),
        comment: comment.id,
        show: () => options.show(comment.id),
        save: (text) => {
          const body: WriteDraftReply = { body: text };
          return postJson<Reply>(urlOf(comment.id), body).then((reply) => {
            // As with a comment, a review may have sent it already.
            if (!options.isAnswered(reply)) setList((all) => [...all, reply]);
          });
        },
      });
    },
    edit(reply: Reply) {
      editor.start({
        key: editKey(reply),
        comment: reply.comment,
        text: reply.body,
        show: () => options.show(reply.comment),
        save: (text) => {
          const body: WriteDraftReply = { body: text };
          return putJson<Reply>(
            `${urlOf(reply.comment)}/${reply.id}`,
            body,
          ).then((edited) =>
            setList((all) =>
              all.map((other) => (other.id === edited.id ? edited : other)),
            ),
          );
        },
      });
    },
    remove(reply: Reply) {
      setDeleting(reply.id);
      setFailure(undefined);
      deleteJson(`${urlOf(reply.comment)}/${reply.id}`)
        .then(
          () => setList((all) => all.filter((other) => other.id !== reply.id)),
          (error: unknown) =>
            setFailure({
              id: reply.id,
              ...failureOf(error),
            }),
        )
        .finally(() => setDeleting(undefined));
    },
    sent,
    /** A review sent every draft reply the hub had for the item. */
    submitted(review: Review) {
      sent(review.replies);
      // The review waited for every editor to be empty.
      for (const reply of review.replies) editor.close(editKey(reply));
    },
  };
}

/** Reply on a sent comment the hub still takes a review for. */
export function ReplyButton(props: { comments: Comments; comment: Comment }) {
  return (
    <Show when={props.comments.replyDrafts.canReply(props.comment)}>
      <span class={styles.actions}>
        <Button onClick={() => props.comments.replyDrafts.reply(props.comment)}>
          Reply
        </Button>
      </span>
    </Show>
  );
}

/** Edit and Delete on a draft reply the hub still takes changes to. */
export function DraftReplyActions(props: { comments: Comments; reply: Reply }) {
  return (
    <Show when={props.comments.replyDrafts.canChange()}>
      <span class={styles.actions}>
        <Button onClick={() => props.comments.replyDrafts.edit(props.reply)}>
          Edit
        </Button>
        <Button
          disabled={props.comments.replyDrafts.deleting() !== undefined}
          onClick={() => props.comments.replyDrafts.remove(props.reply)}
        >
          Delete
        </Button>
      </span>
    </Show>
  );
}

function urlOf(comment: string): string {
  return `/api/comments/${encodeURIComponent(comment)}/drafts`;
}
