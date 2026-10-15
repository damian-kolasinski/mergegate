// Timing for the budget tests (tech-stack.md §9): from what the operator did
// to the frame that shows its result, painted. Measured everywhere, enforced
// with MG_ENFORCE_BUDGETS=1, which CI sets on Ubuntu only.

import type { Locator, Page, TestInfo } from "@playwright/test";
import { expect } from "./hub";

/**
 * What the page reads to tell the result is on screen: the first element
 * matching `selector`, for its text or for whether there is one, or every
 * one, for how many there are or for all their text. Shadow roots included.
 */
export type Probe = {
  selector: string;
  read: "text" | "found" | "count" | "texts";
};

type Reading = string | number | boolean | null;

/** What the timing keeps in the page, on `performance.now()`'s clock. */
type Timed = {
  /** When the operator acted. */
  started?: number;
  /** Milliseconds from then to the result, painted. */
  took?: number;
  reads?: (probe: Probe) => Reading;
};

/**
 * From pressing `key` to the end of the first frame in which `probe` reads
 * other than it did before. Timed from the event's own timestamp, so the
 * browser's input queue counts too: what Event Timing, and so INP, call the
 * interaction's duration. That includes the wait for the display's next
 * frame, up to 16.7 ms at 60 Hz whatever the app does. The page can't time
 * the app's work without it: Chrome renders the frame before it runs
 * anything the key's handlers queue.
 */
export async function pressToFrame(
  page: Page,
  key: string,
  probe: Probe,
): Promise<number> {
  await watch(page, probe);
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
  return took(page);
}

/**
 * From the page sending `body` to `path`, as another client of the hub
 * would, to the end of the first frame in which `probe` reads other than it
 * did before: the hub's work, its event, and the page's refetch and render.
 */
export async function sendToFrame(
  page: Page,
  path: string,
  body: unknown,
  probe: Probe,
): Promise<number> {
  await watch(page, probe);
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
  return took(page);
}

/**
 * From clicking `target` to the end of the first frame in which `probe`
 * reads other than it did before. Timed from the click's own timestamp, as
 * `pressToFrame` times a key.
 */
export async function clickToFrame(
  page: Page,
  target: Locator,
  probe: Probe,
): Promise<number> {
  // Scrolled to first, so the click's own scroll isn't timed.
  await target.scrollIntoViewIfNeeded();
  await watch(page, probe);
  await page.evaluate(() => {
    addEventListener(
      "click",
      (event) => {
        (window as Timed).started = event.timeStamp;
      },
      { capture: true, once: true },
    );
  });
  await target.click();
  return took(page);
}

/**
 * Starts reading `probe` after every frame the page paints, until it reads
 * otherwise than now with the operator's action started. Read in the page,
 * from before the action: Playwright's own round trips would otherwise land
 * in the time.
 */
async function watch(page: Page, probe: Probe): Promise<void> {
  // Installed as a function rather than sent as a string: the hub's CSP
  // forbids eval.
  await page.evaluate((probe) => {
    const timed = window as Timed;
    delete timed.started;
    delete timed.took;
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
      const all = (root: ParentNode): Element[] => [
        ...root.querySelectorAll(selector),
        ...[...root.querySelectorAll("*")].flatMap((host) =>
          host.shadowRoot ? all(host.shadowRoot) : [],
        ),
      ];
      if (read === "count") return all(document).length;
      if (read === "texts") {
        return all(document)
          .map((found) => found.textContent)
          .join("\n");
      }
      if (read === "found") return find(document) !== null;
      return find(document)?.textContent ?? null;
    };
    const reads = timed.reads;
    const before = reads(probe);
    // A message posted from a frame's callbacks arrives once the frame is
    // rendered, so the time covers the view's own frame callbacks, layout
    // and paint. The clock is read before the probe, whose walk of a large
    // diff's shadow roots is the test's time, not the app's.
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      const now = performance.now();
      const { started } = timed;
      if (started !== undefined && reads(probe) !== before) {
        timed.took = now - started;
        channel.port1.close();
      } else {
        requestAnimationFrame(next);
      }
    };
    const next = () => channel.port2.postMessage(null);
    requestAnimationFrame(next);
  }, probe);
}

async function took(page: Page): Promise<number> {
  const done = await page.waitForFunction(() => (window as Timed).took);
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
  const description = `median ${at(0.5).toFixed(1)} ms, p95 ${at(0.95).toFixed(1)} ms, max ${at(1).toFixed(1)} ms over ${sorted.length}; budget ${budget} ms`;
  info.annotations.push({ type: what, description });
  // CI keeps the report only on failure; its log keeps a passing margin.
  console.log(`${what}: ${description}`);
  if (process.env.MG_ENFORCE_BUDGETS === "1") {
    expect(at(0.5), what).toBeLessThan(budget);
  }
}
