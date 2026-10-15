import { createMemo, For, Match, Show, Switch } from "solid-js";
import type { Comment } from "../../api/Comment";
import type { Spot } from "../../diff-view/diff-view";
import { Badge, ErrorNote, Meta } from "../../design-system";
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
 * A file unchanged since the revision compared against stays folded out of
 * the diff, with its threads in the Conversation list, unless one of them
 * waits on the operator: that brings the file back into the diff, to stay.
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
  unchanged: {
    /** The revision the diff compares against; null for all changes. */
    since: () => number | null;
    paths: () => ReadonlySet<string>;
    waiting: (comment: Comment) => boolean;
  },
) {
  const all = createMemo(() =>
    [...lists.sent(), ...lists.drafts()].sort(
      (a, b) => numberOf(a) - numberOf(b),
    ),
  );
  const onSide = (comment: Comment) => {
    const spot = spotOf(comment, revision);
    return spot && onScreen(spot) ? spot : undefined;
  };
  // An unfolded file stays in the diff while the screen compares against
  // the same revision, though its thread stops waiting: settled from its
  // card there, or answered in a sync, it would otherwise take the file and
  // the operator's place in it away.
  const kept = createMemo<{ since: number | null; paths: ReadonlySet<string> }>(
    (before) => {
      const since = unchanged.since();
      const paths: ReadonlySet<string> =
        before?.since === since ? before.paths : new Set();
      const back = all().flatMap((comment) => {
        const spot = onSide(comment);
        return spot &&
          !paths.has(spot.path) &&
          unchanged.paths().has(spot.path) &&
          unchanged.waiting(comment)
          ? [spot.path]
          : [];
      });
      if (before && paths === before.paths && !back.length) return before;
      return { since, paths: new Set([...paths, ...back]) };
    },
  );
  const unfolded = () => kept().paths;
  const inDiff = (comment: Comment) => {
    const spot = onSide(comment);
    return (
      spot !== undefined &&
      (!unchanged.paths().has(spot.path) || unfolded().has(spot.path))
    );
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
    /**
     * The unchanged files a waiting thread brought into the diff since the
     * screen began comparing against the revision it does.
     */
    unfolded,
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
                fallback={<Badge tone="success">Sent to agent</Badge>}
              >
                <Badge
                  tone="warning"
                  title="Not sent yet: it goes out with your review"
                >
                  Draft
                </Badge>
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
          {(failure) => <ErrorNote failure={failure()} />}
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
