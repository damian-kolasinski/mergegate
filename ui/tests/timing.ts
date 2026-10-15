// Timing for the budget tests (tech-stack.md §9): from what the operator did
// to the frame that shows its result. Measured everywhere, enforced with
// MG_ENFORCE_BUDGETS=1, which CI sets on Ubuntu only.

import type { Page, TestInfo } from "@playwright/test";
import { expect } from "./hub";

/**
 * What the page reads to tell the result is on screen: the first element
 * matching `selector`, shadow roots included, for its text or for whether
 * there is one, or how many match outside shadow roots.
 */
export type Probe = { selector: string; read: "text" | "found" | "count" };

type Reading = string | number | boolean | null;

/** What the timing keeps in the page. */
type Timed = {
  started?: number;
  reads?: (probe: Probe) => Reading;
};

/**
 * Milliseconds from pressing `key` to the first animation frame in which
 * `probe` reads other than it did before: the DOM holding the result, which
 * the browser then paints in that frame. Timed from the event's own
 * timestamp, so the browser's input queue counts too.
 */
export async function pressToFrame(
  page: Page,
  key: string,
  probe: Probe,
): Promise<number> {
  const before = await start(page, probe);
  await page.evaluate(() => {
    addEventListener(
      "keydown",
      (event) => {
        (window as Timed).started = event.timeStamp;
      },
      { capture: true, once: true },
    );
  });
  await page.keyboard.press(key);
  return untilFrame(page, probe, before);
}

/**
 * Milliseconds from the page sending `body` to `path`, as another client of
 * the hub would, to the first frame in which `probe` reads other than it did
 * before: the hub's work, its event, and the page's refetch and render.
 */
export async function sendToFrame(
  page: Page,
  path: string,
  body: unknown,
  probe: Probe,
): Promise<number> {
  const before = await start(page, probe);
  await page.evaluate(
    ([path, body]) => {
      (window as Timed).started = performance.now();
      void fetch(path, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "x-mergegate-client": "ui",
        },
        body: JSON.stringify(body),
      });
    },
    [path, body] as const,
  );
  return untilFrame(page, probe, before);
}

/** Readies the page for a timing and answers what `probe` reads now. */
async function start(page: Page, probe: Probe): Promise<Reading> {
  // Installed as a function rather than sent as a string: the hub's CSP
  // forbids eval.
  return page.evaluate((probe) => {
    const timed = window as Timed;
    delete timed.started;
    timed.reads ??= ({ selector, read }) => {
      const find = (root: ParentNode): Element | null => {
        const found = root.querySelector(selector);
        if (found) return found;
        for (const host of root.querySelectorAll("*")) {
          const inner = host.shadowRoot && find(host.shadowRoot);
          if (inner) return inner;
        }
        return null;
      };
      if (read === "count") return document.querySelectorAll(selector).length;
      if (read === "found") return find(document) !== null;
      return find(document)?.textContent ?? null;
    };
    return timed.reads(probe);
  }, probe);
}

async function untilFrame(
  page: Page,
  probe: Probe,
  before: Reading,
): Promise<number> {
  // Checked every frame, so the time is that of the frame with the result.
  const done = await page.waitForFunction(
    ([probe, before]) => {
      const { started, reads } = window as Timed;
      // Read before probing: a probe walking every shadow root of a 5k-line
      // diff takes a while, and that is the test's time, not the app's.
      const now = performance.now();
      return (
        started !== undefined && reads?.(probe) !== before && now - started
      );
    },
    [probe, before] as const,
    { polling: "raf" },
  );
  return (await done.jsonValue()) as number;
}

/** Notes `samples`' spread on the test and holds their median to `budget`. */
export function report(
  info: TestInfo,
  what: string,
  samples: number[],
  budget: number,
) {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.floor((sorted.length - 1) * q)] ?? 0;
  info.annotations.push({
    type: what,
    description: `median ${at(0.5).toFixed(1)} ms, p95 ${at(0.95).toFixed(1)} ms, max ${at(1).toFixed(1)} ms over ${sorted.length}; budget ${budget} ms`,
  });
  if (process.env.MG_ENFORCE_BUDGETS === "1") {
    expect(at(0.5), what).toBeLessThan(budget);
  }
}
