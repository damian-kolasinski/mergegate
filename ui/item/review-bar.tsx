import { createEffect, createSignal, onCleanup, Show } from "solid-js";
import type { Approve } from "../api/Approve";
import type { ChangedSince } from "../api/ChangedSince";
import type { ItemState } from "../api/ItemState";
import type { Merge } from "../api/Merge";
import type { Review } from "../api/Review";
import type { SubmitReview } from "../api/SubmitReview";
import type { Verdict } from "../api/Verdict";
import { type Failure, failureOf, HubError, postJson } from "../app/hub";
import { useKeys } from "../app/keys";
import { Button, ErrorNote, Meta, Mono, TextArea } from "../design-system";
import { NextItem } from "./next";
import styles from "./review-bar.module.css";

/**
 * Sends the operator's review of one revision to its agent, with every draft
 * comment and reply on the item. Send comments leaves the item with the
 * operator; Request changes hands it back; Approve & merge lands it on the
 * target, after a confirm step.
 */
export function ReviewBar(props: {
  item: string;
  revision: number;
  target: string;
  /** The item's state on the hub, which the Approve button follows. */
  state: ItemState;
  /**
   * Set for an item on GitHub, whose merge also closes its pull request:
   * its number, as `#42`, and branch.
   */
  pullRequest: { label: string; branch: string } | undefined;
  /** Open items this one conflicts with, which the confirm step names. */
  conflictsWith: readonly string[];
  /** The newest revision's merge is held or conflicts. */
  blocked: "held" | "conflict" | undefined;
  /** A newer revision was submitted: only it can be approved. */
  behind: boolean;
  /**
   * What changed since the last review; when that was an approval that
   * stands, the bar says so, so approving again is the one step left.
   */
  changed: ChangedSince | null;
  /** Files not marked viewed, which the confirm step warns about. */
  unviewed: number;
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
}) {
  const summary = () => props.summary;
  const setSummary = (summary: string) => props.onSummary(summary);
  const [sending, setSending] = createSignal<"review" | "merge">();
  const [failure, setFailure] = createSignal<Failure>();
  // What the last review that went out carried, with the way on.
  const [sent, setSent] = createSignal<string>();
  const [confirming, setConfirming] = createSignal(false);
  let field: HTMLTextAreaElement | undefined;

  const failed = (error: unknown) => {
    setFailure(failureOf(error));
    if (error instanceof HubError && error.code === "drafts_unplaced") {
      props.onHeld();
    }
  };

  const submit = (verdict: Verdict) => {
    if (held() || empty()) return;
    const body: SubmitReview = {
      revision: props.revision,
      verdict,
      summary: summary().trim(),
    };
    setSending("review");
    setFailure(undefined);
    setSent(undefined);
    postJson<Review>(
      `/api/items/${encodeURIComponent(props.item)}/reviews`,
      body,
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
  // Either can come after the confirm opens, which then stays without its
  // Merge: a comment started from it, or a newer revision. A held merge
  // waits on Retry or Cancel above, not on a second approve.
  const mergeable = () => !held() && !props.behind && props.blocked !== "held";

  const approve = () => {
    if (!mergeable()) return;
    const body: Approve = {
      revision: props.revision,
      summary: summary().trim(),
    };
    setConfirming(false);
    setSending("merge");
    setFailure(undefined);
    setSent(undefined);
    postJson<Merge>(
      `/api/items/${encodeURIComponent(props.item)}/approve`,
      body,
    )
      .then((merge) => {
        setSummary("");
        props.onMerged(merge);
      }, failed)
      .finally(() => setSending(undefined));
  };
  const confirm = () => {
    if (!mergeable()) return;
    // The way on takes Enter, which is the confirm step's too.
    setSent(undefined);
    setConfirming(true);
  };
  const approvedBefore = () =>
    props.changed?.approved ? props.changed : undefined;
  const unlessWriting = (action: () => void) => () => {
    if (!props.writing) action();
  };

  useKeys(
    { r: unlessWriting(() => field?.focus()), a: unlessWriting(confirm) },
    {
      group: "Review",
      keys: [
        ["r", "Write the review's summary"],
        ["⌘↩", "Send comments, from the summary"],
        ["a", "Approve & merge, after a confirm"],
      ],
    },
  );
  // Escape backs out of the confirm step rather than leaving the item; the
  // screen binds its Escape first, so this one listens ahead of it.
  createEffect(() => {
    if (!confirming()) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      // A dialog over the screen, such as the demo viewer, closes first.
      if (document.querySelector("[role=dialog][aria-modal=true]")) return;
      event.preventDefault();
      setConfirming(false);
    };
    window.addEventListener("keydown", onKey, true);
    onCleanup(() => window.removeEventListener("keydown", onKey, true));
  });

  const disabledWhy = () =>
    props.holding
      ? "Save or cancel the open comment first"
      : props.unplaced
        ? "Re-place or delete the drafts that can't move first"
        : empty()
          ? "Write a summary or a comment first"
          : undefined;
  const approveWhy = () =>
    props.behind
      ? `Show r${props.revision + 1} to approve it`
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
            onSubmit={() => submit("comment")}
            ref={(element) => {
              field = element;
            }}
          />
        </div>
        <div class={styles.actions}>
          <Show
            when={confirming()}
            fallback={
              <>
                <Button
                  disabled={held() || empty()}
                  busy={sending() === "review"}
                  title={disabledWhy()}
                  keyHint="⌘↩"
                  onClick={() => submit("comment")}
                >
                  {sending() === "review" ? "Sending…" : "Send comments"}
                </Button>
                <Button
                  // The hub refuses changes requested on a revision the
                  // agent already replaced.
                  disabled={held() || empty() || props.behind}
                  title={
                    props.behind
                      ? `Show r${props.revision + 1} to request changes`
                      : disabledWhy()
                  }
                  onClick={() => submit("changes_requested")}
                >
                  Request changes
                </Button>
                <Button
                  variant={leadsWithApprove() ? "solid" : "outline"}
                  disabled={!mergeable()}
                  busy={sending() === "merge"}
                  title={approveWhy()}
                  keyHint="a"
                  onClick={confirm}
                >
                  {sending() === "merge" ? "Merging…" : "Approve & merge"}
                </Button>
              </>
            }
          >
            <fieldset class={styles.confirm} aria-label="Confirm merge">
              <span class={styles.question}>
                <Show
                  when={props.pullRequest}
                  fallback={
                    <>
                      Squash r{props.revision} onto <Mono>{props.target}</Mono>?
                    </>
                  }
                >
                  {(pull) => (
                    <>
                      Squash-merge PR {pull().label} into{" "}
                      <Mono>{props.target}</Mono> on GitHub and delete{" "}
                      <Mono>{pull().branch}</Mono>?
                    </>
                  )}
                </Show>
                <Show when={props.unviewed}>
                  <Meta tone="removed">
                    {plural(props.unviewed, "file")} not reviewed yet.
                  </Meta>
                </Show>
                <Show when={props.conflictsWith.length}>
                  <Meta>
                    Conflicts with {props.conflictsWith.join(", ")}, which go
                    back to their agents once this lands.
                  </Meta>
                </Show>
              </span>
              <Button onClick={() => setConfirming(false)} keyHint="Esc">
                Cancel
              </Button>
              <Button
                variant="solid"
                disabled={!mergeable()}
                onClick={approve}
                keyHint="↵"
                ref={(element) => requestAnimationFrame(() => element.focus())}
              >
                Merge
              </Button>
            </fieldset>
          </Show>
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
