import { createSignal, onCleanup, Show } from "solid-js";
import type { JSX } from "solid-js";
import type { Approve } from "../api/Approve";
import type { Blocked } from "../api/Blocked";
import type { ChangedSince } from "../api/ChangedSince";
import type { ItemState } from "../api/ItemState";
import type { Merge } from "../api/Merge";
import type { Review } from "../api/Review";
import type { SubmitReview } from "../api/SubmitReview";
import { type Failure, failureOf, HubError, postJson } from "../app/hub";
import { isEditable, isSubmitKey, useKeys } from "../app/keys";
import { Button, ErrorNote, Meta, TextArea } from "../design-system";
import { NextItem } from "./next";
import { leaveReview, settleReviewTime } from "./review-time";
import styles from "./review-bar.module.css";

/**
 * Sends the operator's review of one revision to its agent, with every draft
 * comment and reply on the item. Send comments hands the item back when it
 * carries new comments or a summary on the newest revision, which the agent
 * acts on, while one with only replies, or only comments on an older
 * revision, leaves it with the operator. Approve & merge lands it on the
 * target in one step: the button is only enabled while that can go through.
 */
export function ReviewBar(props: {
  item: string;
  revision: number;
  target: string;
  /** The item's state on the hub, which the Approve button follows. */
  state: ItemState;
  /** Why the newest revision's merge didn't land, while it stands. */
  blocked: Blocked["kind"] | undefined;
  /**
   * The open item this one is stacked on: approving holds the merge until
   * that one lands, then lands this right after it.
   */
  after: string | undefined;
  /** The newer revision submitted, if any: only it can be approved. */
  newest: number | undefined;
  /**
   * What changed since the last review; when that was an approval that
   * stands, the bar says so, so approving again is the one step left.
   */
  changed: ChangedSince | null;
  drafts: number;
  /**
   * Drafts from earlier revisions whose lines changed: the review waits until
   * each is re-placed or deleted.
   */
  unplaced: number;
  /** The draft whose new lines the operator is picking in the diff. */
  placing: string | undefined;
  onStopPlacing: () => void;
  /** Draft replies on sent comments. */
  replies: number;
  /** A comment being written, which the review would go out without. */
  holding: boolean;
  /** An editor is open: `r` and `a` would pull focus away from it. */
  writing: boolean;
  /** What's typed as the review's summary, which outlives the bar. */
  summary: string;
  onSummary: (summary: string) => void;
  onSubmitted: (review: Review) => void;
  /** The hub held the review over drafts that can't move to this revision. */
  onHeld: () => void;
  onMerged: (merge: Merge) => void;
  /**
   * Notes on the item that only inform, first in the status row: no agent
   * to read what's sent, a newer revision to switch to.
   */
  notes?: JSX.Element;
}) {
  const summary = () => props.summary;
  const setSummary = (summary: string) => props.onSummary(summary);
  const [sending, setSending] = createSignal<"review" | "merge">();
  const [failure, setFailure] = createSignal<Failure>();
  // What the last review that went out carried, with the way on.
  const [sent, setSent] = createSignal<string>();
  let field: HTMLTextAreaElement | undefined;

  const failed = (error: unknown) => {
    setFailure(failureOf(error));
    if (error instanceof HubError && error.code === "drafts_unplaced") {
      props.onHeld();
    }
  };

  const submit = () => {
    if (held() || empty()) return;
    const body: SubmitReview = {
      revision: props.revision,
      verdict: "comment",
      summary: summary().trim(),
    };
    setSending("review");
    setFailure(undefined);
    setSent(undefined);
    // A review ends the operator's turn, so the time up to it is that turn's.
    settleReviewTime()
      .then(() =>
        postJson<Review>(
          `/api/items/${encodeURIComponent(props.item)}/reviews`,
          body,
        ),
      )
      .then((review) => {
        setSummary("");
        const parts = [
          review.comments.length && plural(review.comments.length, "comment"),
          review.replies.length && plural(review.replies.length, "reply"),
        ].filter(Boolean);
        setSent(
          // The header's state says whether changes were requested.
          `Sent to the agent on r${body.revision}` +
            (parts.length ? ` with ${parts.join(" and ")}` : ""),
        );
        props.onSubmitted(review);
      }, failed)
      .finally(() => setSending(undefined));
  };
  const empty = () => !summary().trim() && !props.drafts && !props.replies;

  // What would go out without what's held: an open comment, or drafts that
  // can't move to this revision.
  const held = () =>
    sending() !== undefined || props.holding || props.unplaced > 0;
  const behind = () => props.newest !== undefined;
  // A held merge waits on Retry or Cancel above, not on a second approve.
  const mergeable = () => !held() && !behind() && props.blocked !== "held";

  const approve = () => {
    if (!mergeable()) return;
    const body: Approve = {
      revision: props.revision,
      summary: summary().trim(),
    };
    setSending("merge");
    setFailure(undefined);
    setSent(undefined);
    const url = `/api/items/${encodeURIComponent(props.item)}/approve`;
    // The time up to the merge counts, and none while it runs.
    leaveReview(() => postJson<Merge>(url, body))
      .then((merge) => {
        setSummary("");
        props.onMerged(merge);
      }, failed)
      .finally(() => setSending(undefined));
  };
  const approvedBefore = () =>
    props.changed?.approved ? props.changed : undefined;
  const unlessWriting = (action: () => void) => () => {
    if (!props.writing) action();
  };

  useKeys(
    {
      r: unlessWriting(() => field?.focus()),
      // `a` merges at once, so a held key's repeats mustn't approve again
      // once a quick merge or conflict hands the button back.
      a: (event) => {
        if (!event.repeat) unlessWriting(approve)();
      },
    },
    {
      group: "Review",
      keys: [
        ["r", "Write the review's summary"],
        ["⌘↩", "Send comments"],
        ["a", "Approve & merge"],
      ],
    },
  );
  // `⌘↩` sends comments from anywhere on the screen, as its hint on the
  // button says, not only from the summary. `useKeys` leaves modified keys
  // to the browser, so it's bound here; a field's own `⌘↩` (the summary's,
  // a comment's) comes first and takes it.
  const onSubmitKey = (event: KeyboardEvent) => {
    if (!isSubmitKey(event) || event.defaultPrevented || event.repeat) return;
    if (isEditable(event.target) || props.writing) return;
    // `⌘↩` on a focused link opens it in a new tab.
    if (event.target instanceof Element && event.target.closest("a[href]")) {
      return;
    }
    // A dialog over the screen, such as the demo viewer, keeps its keys.
    if (document.querySelector("[role=dialog][aria-modal=true]")) return;
    event.preventDefault();
    submit();
  };
  window.addEventListener("keydown", onSubmitKey);
  onCleanup(() => window.removeEventListener("keydown", onSubmitKey));

  const disabledWhy = () =>
    props.holding
      ? "Save or cancel the open comment first"
      : props.unplaced
        ? "Re-place or delete the drafts that can't move first"
        : empty()
          ? "Write a summary or a comment first"
          : undefined;
  const approveWhy = () =>
    behind()
      ? `Show r${props.newest} to approve it`
      : props.blocked === "held"
        ? "The merge is held: retry or cancel it above"
        : props.blocked === "conflict"
          ? `Conflicts with ${props.target}: merging fails until the agent rebases`
          : props.holding
            ? "Save or cancel the open comment first"
            : undefined;
  // With the agent, or blocked, approving isn't what the screen leads with.
  const leadsWithApprove = () =>
    props.state === "needs_review" && props.blocked === undefined;

  return (
    <form class={styles.bar} aria-label="Review">
      <div class={styles.status}>
        {props.notes}
        <p role="status" class={styles.said}>
          <Show when={failure()}>
            {(current) => <ErrorNote failure={current()} />}
          </Show>
          <Show when={!failure() && sent()}>
            {(lead) => <NextItem after={props.item} lead={`${lead()}.`} />}
          </Show>
        </p>
        <Show when={approvedBefore()}>
          {(changed) => <Meta>{approvedLabel(changed())}</Meta>}
        </Show>
        <Show when={props.drafts}>
          <Meta>{plural(props.drafts, "draft comment")}</Meta>
        </Show>
        <Show when={props.replies}>
          <Meta>{plural(props.replies, "draft reply")}</Meta>
        </Show>
        <Show when={props.holding}>
          <Meta>Save or cancel the open comment first</Meta>
        </Show>
        <Show
          when={props.placing}
          fallback={
            <Show when={props.unplaced}>
              <ErrorNote
                message={`${plural(props.unplaced, "draft")} can't move to r${props.revision}`}
                hint={`re-place or delete ${props.unplaced === 1 ? "it" : "them"}`}
              />
            </Show>
          }
        >
          {(placing) => (
            <>
              <Meta>Pick the lines for {placing()} in the diff</Meta>
              <Button onClick={() => props.onStopPlacing()}>Cancel</Button>
            </>
          )}
        </Show>
      </div>
      <div class={styles.row}>
        <div class={styles.summary}>
          <TextArea
            label="Review summary"
            placeholder="Summary for the agent — press r to write, ⌘↩ to send comments"
            value={summary()}
            // What's typed while sending would be cleared with the rest.
            disabled={sending() !== undefined}
            onInput={setSummary}
            onSubmit={() => submit()}
            ref={(element) => {
              field = element;
            }}
          />
        </div>
        <div class={styles.actions}>
          <Button
            disabled={held() || empty()}
            busy={sending() === "review"}
            title={disabledWhy()}
            keyHint="⌘↩"
            onClick={() => submit()}
          >
            {sending() === "review" ? "Sending…" : "Send comments"}
          </Button>
          <Button
            variant={leadsWithApprove() ? "solid" : "outline"}
            disabled={!mergeable()}
            busy={sending() === "merge"}
            title={approveWhy()}
            keyHint="a"
            onClick={approve}
          >
            {sending() === "merge"
              ? "Merging…"
              : props.after
                ? `Approve, merge after ${props.after}`
                : "Approve & merge"}
          </Button>
        </div>
      </div>
    </form>
  );
}

function approvedLabel(changed: ChangedSince): string {
  // It stands yet didn't carry, so something changed even when no file to
  // review did: a trusted one, or one the rebase conflicted on.
  const files = changed.files ? `${plural(changed.files, "file")} ` : "";
  return `Approved r${changed.revision} · ${files}changed since`;
}

function plural(count: number, noun: string): string {
  if (count === 1) return `${count} ${noun}`;
  return `${count} ${noun.endsWith("y") ? `${noun.slice(0, -1)}ies` : `${noun}s`}`;
}
