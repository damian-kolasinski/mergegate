import { For, Show } from "solid-js";
import type { Comment } from "../api/Comment";
import { Badge, Button, Meta, Mono } from "../design-system";
import type { Comments } from "./comments/comments";
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
        </Button>
      </span>
      <Show when={props.open}>
        <ConversationList comments={props.comments} onShow={props.onShow} />
      </Show>
    </>
  );
}

/**
 * Every comment on the item as `path:line @rN` with where the agent left it.
 * The diff shows those whose lines it has, its own revision's or ones that
 * followed their lines to it, so the rest are answered here.
 */
function ConversationList(props: {
  comments: Comments;
  /** Called once a comment the diff has shows under its line. */
  onShow: () => void;
}) {
  return (
    <aside id="conversation" class={styles.panel} aria-label="Conversation">
      <Show
        when={props.comments.threads.all().length}
        fallback={<Meta>No comments yet. Press c on a line to write one.</Meta>}
      >
        <ol class={styles.list}>
          <For each={props.comments.threads.all()}>
            {(comment) => (
              <Entry
                comments={props.comments}
                comment={comment}
                onShow={() => {
                  props.comments.show(comment);
                  props.onShow();
                }}
              />
            )}
          </For>
        </ol>
      </Show>
    </aside>
  );
}

function Entry(props: {
  comments: Comments;
  comment: Comment;
  onShow: () => void;
}) {
  const inDiff = () => props.comments.threads.inDiff(props.comment);
  const replies = () => props.comments.thread(props.comment).length;
  return (
    <li class={styles.entry} aria-label={props.comment.id}>
      <header class={styles.where}>
        <Mono>{whereOf(props.comment)}</Mono>
        {/* Until the agent answers, the comment's own badge says where it
            is, and the full card below carries one. */}
        <Show
          when={props.comments.replies.answer(props.comment)}
          fallback={
            <Show when={inDiff()}>
              <Show
                when={props.comments.drafts.isDraft(props.comment)}
                fallback={
                  <Show
                    when={props.comment.external?.pending}
                    fallback={<Badge tone="success">Sent to agent</Badge>}
                  >
                    <Badge tone="warning">Not sent yet</Badge>
                  </Show>
                }
              >
                <Badge tone="warning">Draft</Badge>
              </Show>
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
          <p class={styles.excerpt}>
            {props.comment.body}
            <Show when={replies()}>
              {(count) => (
                <Meta>
                  {" "}
                  · {count()} {count() === 1 ? "reply" : "replies"}
                </Meta>
              )}
            </Show>
          </p>
        }
      >
        <CommentCard comments={props.comments} comment={props.comment} />
      </Show>
    </li>
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
