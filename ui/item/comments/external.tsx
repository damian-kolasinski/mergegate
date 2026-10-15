import { createSignal, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { ExternalComment } from "../../api/ExternalComment";
import type { OverrideRejection } from "../../api/OverrideRejection";
import type { Reply } from "../../api/Reply";
import { type Failure, failureOf, postJson } from "../../app/hub";
import {
  Badge,
  Button,
  ErrorNote,
  ExternalLink,
  Meta,
} from "../../design-system";
import type { Comments } from "./comments";
import { Editor, type CommentEditor } from "./editor";
import type { Replies } from "./replies";
import styles from "./external.module.css";
import type { Sent } from "./sent";

export type External = ReturnType<typeof useExternal>;

/**
 * The operator's say on the agent's rejection of a comment from GitHub:
 * agreeing closes it, overriding sends the agent back to it at once.
 */
export function useExternal(options: {
  editor: CommentEditor;
  reviewable: () => boolean;
  /** Puts the comment `id` back on the page. */
  show: (id: string) => void;
  sent: Sent;
  replies: Replies;
}) {
  const { editor } = options;
  const [agreeing, setAgreeing] = createSignal<string>();
  const [failure, setFailure] = createSignal<{ id: string } & Failure>();
  editor.onReset(() => setFailure(undefined));
  const overrideKey = (comment: string) => `override ${comment}`;
  /**
   * The agent's rejection, while it's the comment's latest reply: one the
   * operator answered is theirs to settle with a review.
   */
  const rejection = (comment: Comment): Reply | undefined => {
    if (!comment.external) return undefined;
    const latest = options.replies.to(comment).at(-1);
    return latest?.author === "agent" && latest.status === "rejected"
      ? latest
      : undefined;
  };
  const agreed = (comment: Comment) => {
    const rejected = rejection(comment);
    return (
      rejected !== undefined && comment.external?.agreed_reply === rejected.id
    );
  };
  return {
    rejection,
    agreed,
    /** A rejection the operator hasn't settled, while the item is open. */
    canSettle: (comment: Comment) =>
      options.reviewable() &&
      rejection(comment) !== undefined &&
      !agreed(comment),
    agreeing,
    failure: (comment: Comment): Failure | undefined => {
      const failed = failure();
      return failed?.id === comment.id ? failed : undefined;
    },
    overriding: (comment: Comment) => editor.isOpen(overrideKey(comment.id)),
    agree(comment: Comment) {
      setAgreeing(comment.id);
      setFailure(undefined);
      postJson<Comment>(`${urlOf(comment.id)}/agree`, {})
        .then(options.sent.update, (error: unknown) =>
          setFailure({ id: comment.id, ...failureOf(error) }),
        )
        .finally(() => setAgreeing(undefined));
    },
    override(comment: Comment) {
      editor.start({
        key: overrideKey(comment.id),
        comment: comment.id,
        optional: true,
        show: () => options.show(comment.id),
        save: (text) => {
          const body: OverrideRejection = { note: text.trim() };
          return postJson<Reply>(`${urlOf(comment.id)}/override`, body).then(
            (reply) => options.replies.add([reply]),
          );
        },
      });
    },
  };
}

/** Who left a comment on GitHub, and whether the hub still holds it. */
export function ExternalMeta(props: { external: ExternalComment }) {
  return (
    <>
      <Meta>{props.external.author}</Meta>
      <Show when={props.external.bot}>
        <Badge>Bot</Badge>
      </Show>
      <Show
        when={props.external.pending}
        fallback={<Badge tone="success">Sent to agent</Badge>}
      >
        {/* Until the reviewers go quiet, so the agent gets them at once. */}
        <Badge
          tone="warning"
          title="Held until the reviewers go quiet, so the agent gets them together"
        >
          Not sent yet
        </Badge>
      </Show>
      <ExternalLink href={props.external.url} title="Open on GitHub">
        GitHub
      </ExternalLink>
    </>
  );
}

/** Whether the operator accepted the agent's rejection of the comment. */
export function RejectionActions(props: {
  comments: Comments;
  comment: Comment;
}) {
  return (
    <Show when={props.comments.external.agreed(props.comment)}>
      <Badge tone="neutral">Rejection accepted</Badge>
    </Show>
  );
}

/**
 * The operator's say on the agent's rejection, on the rejection itself:
 * accept it, or override it and send the agent back to the comment.
 */
export function SettleRejection(props: {
  comments: Comments;
  comment: Comment;
}) {
  const external = () => props.comments.external;
  return (
    <Show when={external().canSettle(props.comment)}>
      <div class={styles.settle}>
        <span class={styles.actions}>
          <Button
            busy={external().agreeing() === props.comment.id}
            disabled={external().agreeing() !== undefined}
            onClick={() => external().agree(props.comment)}
          >
            Accept rejection
          </Button>
          <Button onClick={() => external().override(props.comment)}>
            Override — must fix
          </Button>
        </span>
        <Meta>
          Accepting closes it · Override sends your reply to the agent now
        </Meta>
      </div>
    </Show>
  );
}

/** The note an override goes out with, and why one failed. */
export function OverrideEditor(props: {
  comments: Comments;
  comment: Comment;
}) {
  return (
    <>
      <Show when={props.comments.external.overriding(props.comment)}>
        <Editor
          editor={props.comments.editor}
          label={`Override the rejection of ${props.comment.id}`}
          placeholder="Why the agent should make the change, if it needs saying; sent at once (⌘↩ sends)"
          action="Send override"
        />
      </Show>
      <Show when={props.comments.external.failure(props.comment)}>
        {(failure) => <ErrorNote failure={failure()} />}
      </Show>
    </>
  );
}

function urlOf(comment: string): string {
  return `/api/comments/${encodeURIComponent(comment)}`;
}
