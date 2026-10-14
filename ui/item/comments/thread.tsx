import { createMemo, For, Show } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { Spot } from "../../diff-view/diff-view";
import { Badge, Meta } from "../../design-system";
import { linesOf, sameSpot, spotOf } from "./anchor";
import type { Comments } from "./comments";
import { DraftActions } from "./drafts";
import { Editor } from "./editor";
import { ReplyList } from "./replies";
import { ReplyButton } from "./reply-drafts";
import styles from "./thread.module.css";

/**
 * Every comment, sent and draft, on every revision. The diff shows the shown
 * revision's under their last line, and the new one being written.
 */
export function useThreads(
  revision: number,
  lists: {
    sent: () => Comment[];
    drafts: () => Comment[];
    composing: () => Spot | undefined;
  },
) {
  const all = createMemo(() =>
    [...lists.sent(), ...lists.drafts()].sort(
      (a, b) => numberOf(a) - numberOf(b),
    ),
  );
  const shown = createMemo(() =>
    all().filter((comment) => comment.revision === revision),
  );
  /** The lines that hold a thread, for the diff to make room under. */
  const spots = createMemo(() => {
    const spots = shown().map(spotOf);
    const composing = lists.composing();
    if (composing) spots.push(composing);
    return spots;
  });
  return {
    /** Oldest first. */
    all,
    spots,
    at: (spot: Spot) =>
      shown().filter((comment) => sameSpot(spotOf(comment), spot)),
  };
}

/** What's under one line of the diff: its comments, then a new one. */
export function Thread(props: { comments: Comments; spot: Spot }) {
  const composing = () => {
    const span = props.comments.drafts.composing();
    return span && sameSpot(span, props.spot) ? span : undefined;
  };
  return (
    <div class={styles.thread}>
      <For each={props.comments.threads.at(props.spot)}>
        {(comment) => (
          <CommentCard comments={props.comments} comment={comment} />
        )}
      </For>
      <Show when={composing()}>
        {(span) => (
          <Editor
            editor={props.comments.editor}
            label={`Comment on ${span().path} ${linesOf(span().start, span().line).toLowerCase()}`}
          />
        )}
      </Show>
    </div>
  );
}

/** A comment with its thread, or its editor while it's being edited. */
export function CommentCard(props: { comments: Comments; comment: Comment }) {
  return (
    <Show
      when={!props.comments.drafts.editing(props.comment)}
      fallback={
        <Editor
          editor={props.comments.editor}
          label={`Edit comment ${props.comment.id}`}
        />
      }
    >
      <article
        class={styles.comment}
        aria-label={`Comment ${props.comment.id}`}
      >
        <header class={styles.meta}>
          <Show
            when={props.comments.drafts.isDraft(props.comment)}
            fallback={<Badge tone="success">Sent</Badge>}
          >
            <Badge tone="warning">Draft</Badge>
          </Show>
          <Meta>{linesOf(props.comment.start_line, props.comment.line)}</Meta>
          <DraftActions comments={props.comments} comment={props.comment} />
          <ReplyButton comments={props.comments} comment={props.comment} />
        </header>
        <p class={styles.body}>{props.comment.body}</p>
        <ReplyList comments={props.comments} comment={props.comment} />
        <Show when={props.comments.drafts.failure(props.comment)}>
          {(message) => (
            <p role="alert">
              <Meta tone="removed">{message()}</Meta>
            </p>
          )}
        </Show>
      </article>
    </Show>
  );
}

/** Comment ids count up hub-wide: `c_17`. */
function numberOf(comment: Comment): number {
  return Number(comment.id.slice(2));
}
