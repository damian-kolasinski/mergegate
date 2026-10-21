import { createSignal, Match, Show, Switch } from "solid-js";
import type { CastVote } from "../../api/CastVote";
import type { Comment } from "../../api/Comment";
import type { ExternalComment } from "../../api/ExternalComment";
import type { OverrideRejection } from "../../api/OverrideRejection";
import type { Reply } from "../../api/Reply";
import type { Vote } from "../../api/Vote";
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
 * The operator's say on comments from GitHub: their vote on one before
 * the agent gets it, and on the agent's rejection of one, where agreeing
 * closes it and overriding sends the agent back to it at once.
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
  const [voting, setVoting] = createSignal<string>();
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
  const fail = (comment: Comment) => (error: unknown) =>
    setFailure({ id: comment.id, ...failureOf(error) });
  return {
    /**
     * A comment from GitHub no batch has carried, while the item is open: a
     * lapsed one too, which the operator may still send.
     */
    canVote: (comment: Comment) => {
      const external = comment.external;
      return (
        options.reviewable() &&
        external !== undefined &&
        (external.pending || external.lapsed || external.vote === "down")
      );
    },
    voting,
    vote(comment: Comment, vote: Vote) {
      setVoting(comment.id);
      setFailure(undefined);
      const body: CastVote = { vote };
      postJson<Comment>(`${urlOf(comment.id)}/vote`, body)
        .then(options.sent.update, fail(comment))
        .finally(() => setVoting(undefined));
    },
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
        .then(options.sent.update, fail(comment))
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
      <ExternalStatus external={props.external} />
      <ExternalLink href={props.external.url} title="Open on GitHub">
        GitHub
      </ExternalLink>
    </>
  );
}

/** A comment the agent won't get until the operator votes on it. */
export function awaitingVote(external: ExternalComment): boolean {
  return external.vote === null && external.pending;
}

/** Whether a comment from GitHub reached the agent, and what it waits on. */
export function ExternalStatus(props: { external: ExternalComment }) {
  return (
    <Switch fallback={<Badge tone="success">Sent to agent</Badge>}>
      <Match when={props.external.summary}>
        <Badge title="A bot's summary, for you to read: the agent gets its findings as comments of their own">
          Summary
        </Badge>
      </Match>
      <Match when={props.external.vote === "down"}>
        <Badge title="You kept it from the agent">Dismissed</Badge>
      </Match>
      <Match when={props.external.lapsed}>
        <Badge title="A later revision outdated its lines before you voted, so it stopped waiting: the agent gets it only if you send it">
          Lapsed
        </Badge>
      </Match>
      <Match when={awaitingVote(props.external)}>
        <Badge
          tone="warning"
          solid
          title="A comment from GitHub reaches the agent only once you vote it up"
        >
          Needs your vote
        </Badge>
      </Match>
      <Match when={props.external.pending}>
        <Badge
          tone="warning"
          title="Held until the reviewers go quiet, so the agent gets them together"
        >
          Not sent yet
        </Badge>
      </Match>
    </Switch>
  );
}

/**
 * The operator's vote on a comment from GitHub: up sends it with the next
 * batch, down keeps it from the agent. Either changes until a batch carries
 * it.
 */
export function VoteActions(props: { comments: Comments; comment: Comment }) {
  const external = () => props.comments.external;
  const voted = () => props.comment.external?.vote;
  const button = (vote: Vote, icon: string, label: string, title: string) => (
    <Button
      variant={voted() === vote ? "solid" : "outline"}
      pressed={voted() === vote}
      busy={external().voting() === props.comment.id}
      disabled={external().voting() !== undefined}
      title={title}
      onClick={() => external().vote(props.comment, vote)}
    >
      <span aria-hidden="true">{icon}</span>
      {label}
    </Button>
  );
  return (
    <Show when={external().canVote(props.comment)}>
      <span class={styles.actions}>
        {button(
          "up",
          "👍",
          "Send to agent",
          "The agent gets it with the next batch",
        )}
        {button("down", "👎", "Dismiss", "The agent never gets it")}
      </span>
    </Show>
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
