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

/**
 * Reports the review time measured so far on the item on screen, before a
 * review or an abandon goes out. The hub counts the time it gets before a
 * review toward the turn that review ends. It refuses time once the item
 * leaves review, so the item's closing event reports the same totals every
 * time it's read.
 */
export function settleReviewTime(): Promise<void> {
  return settleShown?.() ?? Promise.resolve();
}

/**
 * Measures how long the operator is active on `item`'s review screen and
 * reports it to the hub with the revision on screen; the hub adds it up per
 * revision and per turn for the item's closing event. Active is the page
 * visible and focused, with the operator's inputs no more than `IDLE_MS`
 * apart: a gap between two counts whole when it's shorter, and not at all
 * when it's longer. Only one window has focus, so an item open in two
 * counts once.
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
  // The operator's last input, while the page is engaged.
  let last: number | undefined;

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
  const input = () => {
    if (shown === undefined || !engaged()) return;
    const now = performance.now();
    credit(now);
    last = now;
  };
  const away = () => {
    credit(performance.now());
    last = undefined;
    void report();
  };
  const visibility = () =>
    document.visibilityState === "visible" ? input() : away();
  // What closes the item is an input itself, so the time up to it counts.
  // A report landing after the review would count toward the next turn, or
  // be refused, so the settle waits for every one on its way, including
  // those a blur or the timer sends while it waits.
  const settle = async () => {
    input();
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
      last = next !== undefined && engaged() ? performance.now() : undefined;
    }),
  );

  const options = { capture: true, passive: true };
  for (const type of inputs) window.addEventListener(type, input, options);
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
    window.removeEventListener("focus", input);
    window.removeEventListener("blur", away);
    window.removeEventListener("pagehide", away);
    document.removeEventListener("visibilitychange", visibility);
    clearInterval(timer);
    if (settleShown === settle) settleShown = undefined;
  });
}
