import {
  createMemo,
  createSignal,
  For,
  type JSX,
  Show,
  untrack,
} from "solid-js";
import type { Comment } from "../api/Comment";
import {
  Badge,
  Button,
  Markdown,
  markdownText,
  Meta,
  Mono,
} from "../design-system";
import type { Comments } from "./comments/comments";
import { awaitingVote, ExternalStatus, VoteActions } from "./comments/external";
import { StatusBadge } from "./comments/replies";
import { CommentCard } from "./comments/thread";
import styles from "./conversation.module.css";

/** The header's Conversation button, and the list it opens. */
export function Conversation(props: {
  comments: Comments;
  open: boolean;
  onToggle: () => void;
  /** Called once a comment the diff has shows under its line. */
  onShow: () => void;
  ref: (button: HTMLButtonElement) => void;
}) {
  // The agent's open questions wait on the operator, so the button says so
  // before the list is opened.
  const questions = () =>
    props.comments.threads
      .all()
      .filter(
        (comment) =>
          props.comments.replies.answer(comment)?.status === "question",
      ).length;
  const votes = () =>
    props.comments.threads
      .all()
      .filter((comment) => comment.external && awaitingVote(comment.external))
      .length;
  return (
    <>
      <span class={styles.talk}>
        <Button
          expanded={props.open}
          controls="conversation"
          ref={(element) => props.ref(element)}
          onClick={() => props.onToggle()}
        >
          Conversation {props.comments.threads.all().length}
          <Show when={questions()}>
            {(count) => (
              <Badge tone="brand" solid>
                {count()} {count() === 1 ? "question" : "questions"}
              </Badge>
            )}
          </Show>
          <Show when={votes()}>
            {(count) => (
              <Badge tone="warning" solid>
                {count()} to vote on
              </Badge>
            )}
          </Show>
        </Button>
      </span>
      <Show when={props.open}>
        <ConversationList comments={props.comments} onShow={props.onShow} />
      </Show>
    </>
  );
}

/**
 * Every comment on the item as `path:line @rN` with where the agent left it,
 * those waiting on the operator first and open, the rest folded under them.
 * The diff shows those whose lines it has, its own revision's or ones that
 * followed their lines to it, so the rest are answered here.
 */
function ConversationList(props: {
  comments: Comments;
  /** Called once a comment the diff has shows under its line. */
  onShow: () => void;
}) {
  // What waited on the operator stays first until the list closes, so what
  // they just did to it, a vote they may take back, stays in sight.
  const kept = createMemo<ReadonlySet<string>>(
    (before) =>
      new Set([
        ...before,
        ...props.comments.threads
          .all()
          .filter((comment) => needsYou(props.comments, comment))
          .map((comment) => comment.id),
      ]),
    new Set(),
  );
  const needed = () =>
    props.comments.threads.all().filter((comment) => kept().has(comment.id));
  // A bot's summary is there to read once, so it goes after what's said on
  // the code.
  const rest = () =>
    props.comments.threads
      .all()
      .filter((comment) => !kept().has(comment.id))
      .sort(
        (a, b) =>
          Number(a.external?.summary ?? false) -
          Number(b.external?.summary ?? false),
      );
  // With nothing waiting on the operator, the rest is all there is to see.
  const [opened, setOpened] = createSignal(untrack(() => !needed().length));
  // An editor open on one of the rest would otherwise go out of sight with
  // what's typed into it when the list is opened again.
  const writing = () => {
    const id = props.comments.editor.open()?.comment;
    return rest().some((comment) => comment.id === id);
  };
  // Kept by id here, not in each entry: a vote replaces the comment, and
  // For would then build its entry anew, folded.
  const [rows, setRows] = createSignal<ReadonlySet<string>>(new Set());
  const entry = (comment: Comment) => (
    <Entry
      comments={props.comments}
      comment={comment}
      needed={kept().has(comment.id)}
      opened={rows().has(comment.id)}
      onOpen={() => setRows((ids) => new Set(ids).add(comment.id))}
      onShow={() => {
        props.comments.show(comment);
        props.onShow();
      }}
    />
  );
  return (
    <aside id="conversation" class={styles.panel} aria-label="Conversation">
      <Show
        when={props.comments.threads.all().length}
        fallback={<Meta>No comments yet. Press c on a line to write one.</Meta>}
      >
        <section class={styles.group} aria-label="Needs you">
          <h3 class={styles.heading}>
            Needs you{" "}
            <span class={styles.count}>
              {
                needed().filter((comment) => needsYou(props.comments, comment))
                  .length
              }
            </span>
          </h3>
          <Show
            when={needed().length}
            fallback={<Meta>Nothing waits on you.</Meta>}
          >
            <ol class={styles.list}>
              <For each={needed()}>{entry}</For>
            </ol>
          </Show>
        </section>
        <Show when={rest().length}>
          <section class={styles.group} aria-label="Everything else">
            <h3 class={styles.heading}>
              <button
                type="button"
                class={styles.toggle}
                aria-expanded={opened() || writing()}
                aria-controls="conversation-rest"
                disabled={writing()}
                onClick={() => setOpened((open) => !open)}
              >
                <Chevron />
                Everything else
                <span class={styles.count}>{rest().length}</span>
              </button>
            </h3>
            <Show when={opened() || writing()}>
              <ol id="conversation-rest" class={styles.list}>
                <For each={rest()}>{entry}</For>
              </ol>
            </Show>
          </section>
        </Show>
      </Show>
    </aside>
  );
}

/**
 * Whether the comment waits on the operator: theirs not sent yet, a reply
 * of theirs not sent yet, one from GitHub awaiting their vote, the agent's
 * question, or the agent's rejection they haven't settled.
 */
function needsYou(comments: Comments, comment: Comment): boolean {
  return (
    comments.drafts.isDraft(comment) ||
    comments.replyDrafts.to(comment).length > 0 ||
    (comment.external !== undefined && awaitingVote(comment.external)) ||
    comments.replies.answer(comment)?.status === "question" ||
    comments.external.canSettle(comment)
  );
}

/**
 * One comment: open when it waits on the operator, otherwise a line that
 * opens to the comment, or goes to it in the diff.
 */
function Entry(props: {
  comments: Comments;
  comment: Comment;
  needed: boolean;
  /** Opened from its line by the operator. */
  opened: boolean;
  onOpen: () => void;
  onShow: () => void;
}) {
  const inDiff = () => props.comments.threads.inDiff(props.comment);
  const replies = () => props.comments.thread(props.comment).length;
  const writing = () =>
    props.comments.editor.open()?.comment === props.comment.id;
  const open = () => props.needed || props.opened || writing();
  const replyCount = () => (
    <Show when={replies()}>
      {(count) => (
        <Meta>
          {" "}
          · {count()} {count() === 1 ? "reply" : "replies"}
        </Meta>
      )}
    </Show>
  );
  return (
    <li class={styles.entry} aria-label={props.comment.id}>
      <header class={styles.where}>
        <Mono>{whereOf(props.comment)}</Mono>
        {/* Until the agent answers, the comment's own badge says where it
            is, and the full card below carries one. */}
        <Show
          when={props.comments.replies.answer(props.comment)}
          fallback={
            <Show when={inDiff() || !open()}>
              <Status comments={props.comments} comment={props.comment} />
            </Show>
          }
        >
          {(answer) => (
            <StatusBadge
              status={answer().status}
              addressedIn={answer().addressed_in}
            />
          )}
        </Show>
        <Show when={inDiff()}>
          <span class={styles.show}>
            <Button onClick={() => props.onShow()}>Show in diff</Button>
          </span>
        </Show>
      </header>
      {/* The diff holds the thread of the comments it shows, and an editor
          open in two places would split what's typed. */}
      <Show
        when={!inDiff()}
        fallback={
          <Show
            when={props.needed}
            fallback={
              <p class={styles.excerpt}>
                {markdownText(props.comment.body)}
                {replyCount()}
              </p>
            }
          >
            <div class={styles.body}>
              <Markdown source={props.comment.body} />
            </div>
            <VoteActions comments={props.comments} comment={props.comment} />
          </Show>
        }
      >
        <Show
          when={open()}
          fallback={
            <button
              type="button"
              class={styles.line}
              aria-expanded="false"
              aria-label={`Open comment ${props.comment.id}`}
              onClick={() => props.onOpen()}
            >
              <span class={styles.excerpt}>
                {markdownText(props.comment.body)}
              </span>
              {replyCount()}
            </button>
          }
        >
          <CommentCard comments={props.comments} comment={props.comment} />
        </Show>
      </Show>
    </li>
  );
}

/** Where a comment is before the agent answers it. */
function Status(props: { comments: Comments; comment: Comment }) {
  return (
    <Show
      when={props.comments.drafts.isDraft(props.comment)}
      fallback={
        <Show
          when={props.comment.external}
          fallback={<Badge tone="success">Sent to agent</Badge>}
        >
          {(external) => <ExternalStatus external={external()} />}
        </Show>
      }
    >
      <Badge tone="warning">Draft</Badge>
    </Show>
  );
}

function Chevron(): JSX.Element {
  return (
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

/**
 * A comment from GitHub on lines no revision's diff shows names only its
 * file, and one on no file the pull request.
 */
function whereOf(comment: Comment): string {
  const at = ` @r${comment.revision}`;
  if (comment.path === null || comment.line === null) {
    return (comment.external?.path ?? "Pull request") + at;
  }
  const lines =
    comment.start_line === comment.line
      ? `${comment.line}`
      : `${comment.start_line}–${comment.line}`;
  const side = comment.side === "old" ? " (old)" : "";
  return `${comment.path}:${lines}${side}${at}`;
}
