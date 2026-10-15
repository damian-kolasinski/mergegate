import { createMemo, For, Match, Show, Switch } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { Spot } from "../../diff-view/diff-view";
import { Badge, Meta } from "../../design-system";
import { linesOf, linesOn, sameSpot, spotOf } from "./anchor";
import type { Comments } from "./comments";
import { DraftActions } from "./drafts";
import { Editor } from "./editor";
import { ExternalMeta, OverrideEditor, RejectionActions } from "./external";
import { ReplyList } from "./replies";
import { ReplyButton } from "./reply-drafts";
import styles from "./thread.module.css";

/**
 * Every comment, sent and draft, on every revision. The diff shows the shown
 * revision's under their last line, the earlier ones' the hub placed onto it
 * under the last of the lines theirs became, and the new one being written.
 */
export function useThreads(
  revision: number,
  lists: {
    sent: () => Comment[];
    drafts: () => Comment[];
    composing: () => Spot | undefined;
  },
  /** Lines of a side the diff on screen has, as an interdiff has no base. */
  onScreen: (spot: Spot) => boolean,
) {
  const all = createMemo(() =>
    [...lists.sent(), ...lists.drafts()].sort(
      (a, b) => numberOf(a) - numberOf(b),
    ),
  );
  const inDiff = (comment: Comment) => {
    const spot = spotOf(comment, revision);
    return spot !== undefined && onScreen(spot);
  };
  const shown = createMemo(() => all().filter(inDiff));
  /** The lines that hold a thread, for the diff to make room under. */
  const spots = createMemo(() => {
    const spots = shown().flatMap((comment) => spotOf(comment, revision) ?? []);
    const composing = lists.composing();
    if (composing) spots.push(composing);
    return spots;
  });
  return {
    /** Oldest first. */
    all,
    /** Shown under its line in the diff, not only in the Conversation list. */
    inDiff,
    spots,
    at: (spot: Spot) =>
      shown().filter((comment) => {
        const at = spotOf(comment, revision);
        return at !== undefined && sameSpot(at, spot);
      }),
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
            when={props.comment.external}
            fallback={
              <Show
                when={props.comments.drafts.isDraft(props.comment)}
                fallback={<Badge tone="success">Sent</Badge>}
              >
                <Badge tone="warning">Draft</Badge>
              </Show>
            }
          >
            {(external) => <ExternalMeta external={external()} />}
          </Show>
          <Show when={linesOn(props.comment, props.comments.revision)}>
            {(lines) => <Meta>{linesOf(lines().start, lines().line)}</Meta>}
          </Show>
          <Placed comment={props.comment} revision={props.comments.revision} />
          <DraftActions comments={props.comments} comment={props.comment} />
          <RejectionActions comments={props.comments} comment={props.comment} />
          <ReplyButton comments={props.comments} comment={props.comment} />
        </header>
        <p class={styles.body}>{props.comment.body}</p>
        <ReplyList comments={props.comments} comment={props.comment} />
        <OverrideEditor comments={props.comments} comment={props.comment} />
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

/**
 * How a comment from an earlier revision sits on the one on screen: on the
 * same lines, on what replaced them, or nowhere its diff shows.
 */
function Placed(props: { comment: Comment; revision: number }) {
  const from = () => props.comment.revision;
  return (
    <Show when={from() < props.revision && props.comment.placement}>
      {(placement) => (
        <Switch>
          <Match when={placement().kind === "moved"}>
            <Meta>from r{from()}</Meta>
          </Match>
          <Match when={placement().kind === "changed"}>
            <Badge tone="warning">Changed since r{from()}</Badge>
          </Match>
          <Match when={placement().kind === "outdated"}>
            <Badge>Outdated</Badge>
          </Match>
        </Switch>
      )}
    </Show>
  );
}

/** Comment ids count up hub-wide: `c_17`. */
function numberOf(comment: Comment): number {
  return Number(comment.id.slice(2));
}
