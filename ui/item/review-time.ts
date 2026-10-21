import { createEffect, on, onCleanup } from "solid-js";
import type { ReviewTime } from "../api/ReviewTime";
import { postOutliving } from "../app/hub";

/**
 * How long a pause in the operator's input still counts as reviewing:
 * reading a hunk can take a while without touching anything. A longer pause
 * counts for nothing, as they may have walked away.
 */
export const IDLE_MS = 2 * 60 * 1000;
/** How often the time measured so far goes to the hub. */
export const REPORT_MS = 30 * 1000;
/** What counts as the operator doing something on the page. */
const inputs = [
  "pointerdown",
  "pointermove",
  "keydown",
  "wheel",
  "touchstart",
  "scroll",
] as const;

// The item screen on view; there's one at a time.
let settleShown: (() => Promise<void>) | undefined;
// Reports on their way, from every screen: one a closed screen sent as it
// went still has to land before a review sent from the next.
const sending = new Set<Promise<void>>();
// Approves and abandons on their way.
let leaving = 0;

/**
 * Reports the review time measured so far on the item on screen, before a
 * review goes out. The hub counts the time it gets before a review toward
 * the turn that review ends.
 */
export function settleReviewTime(): Promise<void> {
  return settleShown?.() ?? Promise.resolve();
}

/**
 * Settles the review time, then sends what takes the item out of review:
 * an approve or an abandon. The hub refuses time once the item is out, so
 * its closing event reports the same totals every time it's read. Nothing
 * counts while the request is on its way either: a merge can take a while,
 * and one that conflicts puts the item back in review on the same revision.
 * Measuring starts again with the operator's next input.
 */
export async function leaveReview<T>(send: () => Promise<T>): Promise<T> {
  leaving += 1;
  try {
    await settleReviewTime();
    return await send();
  } finally {
    leaving -= 1;
  }
}

/**
 * Measures how long the operator is active on `item`'s review screen and
 * reports it to the hub with the revision on screen; the hub adds it up per
 * revision and per turn for the item's closing event. Active is the page
 * visible and focused, with the operator's inputs no more than `IDLE_MS`
 * apart: a gap between two counts whole when it's shorter, and not at all
 * when it's longer. A demo video playing counts as input too, until it has
 * played its length since the operator's last input: watching one takes no
 * input, but one left looping counts no further. Only one window has focus,
 * so an item open in two counts once.
 *
 * `revision` is the revision on screen; undefined while there's none, or
 * the item isn't in review.
 */
export function useReviewTime(
  item: string,
  revision: () => number | undefined,
): void {
  let shown: number | undefined;
  // Measured and not yet reported, in milliseconds.
  let pending = 0;
  // The operator's last activity, while the page is engaged: an input, or a
  // tick of a video playing.
  let last: number | undefined;
  // The operator's last input itself, not a video playing on.
  let touched: number | undefined;

  const engaged = () =>
    document.visibilityState === "visible" && document.hasFocus();
  const credit = (now: number) => {
    if (last !== undefined && now - last <= IDLE_MS) pending += now - last;
  };
  const report = (): Promise<void> => {
    const ms = Math.round(pending);
    pending = 0;
    if (shown === undefined || ms < 1) return Promise.resolve();
    const body: ReviewTime = { revision: shown, ms };
    // Measuring never gets in the operator's way: a report the hub refuses
    // or never gets is dropped.
    const sent = postOutliving(
      `/api/items/${encodeURIComponent(item)}/review-time`,
      body,
    ).catch(() => undefined);
    sending.add(sent);
    void sent.then(() => sending.delete(sent));
    return sent;
  };
  const active = () => {
    if (leaving > 0) {
      last = undefined;
      return;
    }
    if (shown === undefined || !engaged()) return;
    const now = performance.now();
    credit(now);
    last = now;
  };
  const input = () => {
    touched = performance.now();
    active();
  };
  // Media events don't bubble, but the window's capture still sees them. A
  // video of no known length, as a WebM recorded without one, counts nothing.
  const playing = (event: Event) => {
    const video = event.target;
    if (!(video instanceof HTMLVideoElement) || video.paused) return;
    const length = (video.duration * 1000) / video.playbackRate;
    if (
      touched !== undefined &&
      Number.isFinite(length) &&
      performance.now() - touched <= length
    ) {
      active();
    }
  };
  const away = () => {
    credit(performance.now());
    last = undefined;
    void report();
  };
  const visibility = () =>
    document.visibilityState === "visible" ? input() : away();
  // What sends the review is an input itself, so the time up to it counts.
  // A report landing after the review would count toward the next turn, or
  // be refused, so the settle waits for every one on its way, including
  // those a blur or the timer sends while it waits.
  const settle = async () => {
    if (leaving > 0) {
      credit(performance.now());
      last = undefined;
    } else input();
    void report();
    while (sending.size > 0) await Promise.all(sending);
  };

  createEffect(
    on(revision, (next) => {
      // The time until now was the revision that was on screen's.
      if (shown !== undefined) {
        credit(performance.now());
        void report();
      }
      shown = next;
      last =
        next !== undefined && leaving === 0 && engaged()
          ? performance.now()
          : undefined;
    }),
  );

  const options = { capture: true, passive: true };
  for (const type of inputs) window.addEventListener(type, input, options);
  window.addEventListener("timeupdate", playing, options);
  window.addEventListener("focus", input);
  window.addEventListener("blur", away);
  window.addEventListener("pagehide", away);
  document.addEventListener("visibilitychange", visibility);
  const timer = setInterval(() => void report(), REPORT_MS);
  settleShown = settle;
  onCleanup(() => {
    away();
    for (const type of inputs) {
      window.removeEventListener(type, input, options);
    }
    window.removeEventListener("timeupdate", playing, options);
    window.removeEventListener("focus", input);
    window.removeEventListener("blur", away);
    window.removeEventListener("pagehide", away);
    document.removeEventListener("visibilitychange", visibility);
    clearInterval(timer);
    if (settleShown === settle) settleShown = undefined;
  });
}
