// Review time: the item screen measures how long the operator is active on
// it, and the merge reports the total for the agent's telemetry, split by
// the turns the operator's reviews end.

import type { Page } from "@playwright/test";
import { expect, Hub, test } from "./hub";

type Stats = {
  review_ms: number;
  turns: { turn: number; verdict: string | null; review_ms: number }[];
  revisions: { revision: number; review_ms: number }[];
};

/**
 * Holds the time reports `hold` picks, counting from 1, until released, and
 * notes which were still held as each review reached the hub: a review must
 * never overtake one.
 */
async function gate(page: Page, hold: (n: number) => boolean) {
  let sent = 0;
  const held = new Map<number, () => void>();
  const reviews: number[][] = [];
  await page.route("**/review-time", async (route) => {
    const n = ++sent;
    if (hold(n)) await new Promise<void>((go) => held.set(n, go));
    await route.continue();
  });
  await page.route("**/reviews", async (route) => {
    if (route.request().method() === "POST") reviews.push([...held.keys()]);
    await route.continue();
  });
  return {
    reviews,
    count: () => sent,
    release(n: number) {
      held.get(n)?.();
      held.delete(n);
    },
  };
}

/** The stats on app#1's `type` event, as the hub's event log has them. */
function closing(hub: Hub, type: string): Stats | undefined {
  const log = hub.mg(["events", "--since", "0"]) as {
    events: { type: string; stats?: Stats }[];
  };
  return log.events.find((event) => event.type === type)?.stats;
}

test("a merge reports the time the operator was active, per turn", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  // Twenty seconds reading between two moves counts whole.
  await page.mouse.move(10, 10);
  await page.clock.fastForward(20_000);
  await page.mouse.move(20, 20);
  // The review ends the first turn.
  await page
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looks close.");
  await page.getByRole("button", { name: "Send comments" }).click();
  await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();

  // Five minutes without a move counts for nothing: they may have left.
  await page.clock.fastForward(5 * 60_000);
  await page.mouse.move(30, 30);
  await page.clock.fastForward(10_000);
  await page.mouse.move(40, 40);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  const stats = closing(hub, "item.merged");
  // What the clicks took in real time on top, not the five minutes away.
  const [first, second] = stats?.turns ?? [];
  expect(stats?.turns.map((turn) => turn.verdict)).toEqual([
    "comment",
    "approved",
  ]);
  expect(first?.review_ms).toBeGreaterThanOrEqual(20_000);
  expect(first?.review_ms).toBeLessThan(30_000);
  expect(second?.review_ms).toBeGreaterThanOrEqual(10_000);
  expect(second?.review_ms).toBeLessThan(20_000);
  expect(stats?.review_ms).toBe(
    (first?.review_ms ?? 0) + (second?.review_ms ?? 0),
  );
  expect(stats?.revisions).toEqual([
    expect.objectContaining({ review_ms: stats?.review_ms }),
  ]);
});

test("a review waits for the time reports still on their way", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  const reports = await gate(page, (n) => n === 1);
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(20_000);
  await page.mouse.move(20, 20);
  // The half-minute report sends the twenty seconds, and is held.
  await page.clock.fastForward(10_000);
  await expect.poll(reports.count).toBe(1);
  await page
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looks close.");
  await page.getByRole("button", { name: "Send comments" }).click();
  await page.waitForTimeout(500);
  expect(reports.reviews).toEqual([]);
  reports.release(1);
  await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();
  expect(reports.reviews).toEqual([[]]);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  const stats = closing(hub, "item.merged");
  expect(stats?.turns[0]?.review_ms).toBeGreaterThanOrEqual(20_000);
});

test("a review waits for a report sent while it waits", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  const reports = await gate(page, () => true);
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(20_000);
  await page
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looks close.");
  await page.getByRole("button", { name: "Send comments" }).click();
  await expect.poll(reports.count).toBe(1);
  // The operator moves to another window while the review waits.
  await page.clock.fastForward(5_000);
  await page.evaluate(() => {
    document.hasFocus = () => false;
    window.dispatchEvent(new Event("blur"));
  });
  await expect.poll(reports.count).toBe(2);
  reports.release(1);
  await page.waitForTimeout(500);
  expect(reports.reviews).toEqual([]);
  reports.release(2);
  await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();
  expect(reports.reviews).toEqual([[]]);
});

test("a review waits for the reports of an earlier visit", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  const reports = await gate(page, (n) => n === 1);
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(10_000);
  await page.mouse.move(20, 20);
  // Leaving the item sends its time, and that report is held.
  await page.goto(`${hub.url}#/`);
  await expect.poll(reports.count).toBe(1);
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(screen.getByText("Needs review")).toBeVisible();
  await page.mouse.move(30, 30);
  await page.clock.fastForward(5_000);
  await page
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looks close.");
  await page.getByRole("button", { name: "Send comments" }).click();
  await page.waitForTimeout(500);
  expect(reports.reviews).toEqual([]);
  reports.release(1);
  await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();
  expect(reports.reviews).toEqual([[]]);
});

test("time while the window has no focus counts for nothing", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(10_000);
  await page.mouse.move(20, 20);
  // The operator is in another window: moves over this one don't count.
  await page.evaluate(() => {
    document.hasFocus = () => false;
    window.dispatchEvent(new Event("blur"));
  });
  await page.mouse.move(30, 30);
  await page.clock.fastForward(60_000);
  await page.mouse.move(40, 40);
  await page.evaluate(() => {
    document.hasFocus = () => true;
    window.dispatchEvent(new Event("focus"));
  });
  await page.clock.fastForward(5_000);
  await page.mouse.move(50, 50);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  const turn = closing(hub, "item.merged")?.turns[0];
  expect(turn?.review_ms).toBeGreaterThanOrEqual(15_000);
  expect(turn?.review_ms).toBeLessThan(25_000);
});

test("an abandon reports the turn under way first", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(15_000);
  await screen.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("button", { name: "Abandon…" }).click();
  const confirm = page.getByRole("form", { name: "Abandon" });
  await confirm.getByRole("textbox", { name: "Reason" }).fill("Not needed.");
  await confirm.getByRole("button", { name: "Abandon app#1" }).click();
  await expect(screen.getByRole("region", { name: "Abandoned" })).toBeVisible();

  const stats = closing(hub, "item.abandoned");
  expect(stats?.turns).toEqual([
    expect.objectContaining({ turn: 1, verdict: null }),
  ]);
  expect(stats?.review_ms).toBeGreaterThanOrEqual(15_000);
  expect(stats?.review_ms).toBeLessThan(25_000);
});

test("time goes to the revision on screen, not the newest", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(10_000);
  await page.mouse.move(20, 20);
  // r2 arrives, but the operator is still reading r1.
  app.revise("Add logout", { "src/logout.rs": "fn logout() {}\n" });
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner).toBeVisible();
  await page.mouse.move(30, 30);
  await page.clock.fastForward(5_000);
  await page.mouse.move(40, 40);
  await banner.getByRole("button", { name: "Show r2" }).click();
  await page.mouse.move(50, 50);
  await page.clock.fastForward(20_000);
  await page.mouse.move(60, 60);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  const stats = closing(hub, "item.merged");
  const [first, second] = stats?.revisions ?? [];
  expect(first?.revision).toBe(1);
  expect(first?.review_ms).toBeGreaterThanOrEqual(15_000);
  expect(first?.review_ms).toBeLessThan(20_000);
  expect(second?.revision).toBe(2);
  expect(second?.review_ms).toBeGreaterThanOrEqual(20_000);
  expect(second?.review_ms).toBeLessThan(25_000);
  expect(stats?.turns).toEqual([
    expect.objectContaining({ turn: 1, verdict: "approved" }),
  ]);
});

test("time while a merge runs counts for nothing, even when it conflicts", async ({
  page,
}) => {
  // Only the merge finds the conflict.
  const hub = new Hub({ MG_TARGET_WATCH: "off" });
  try {
    const app = hub.project("app");
    app.seed({ README: "hello\n" });
    app.submit("readme", "Greet", { README: "hello, login\n" });
    let merge = () => {};
    const merging = new Promise<void>((go) => {
      merge = go;
    });
    await page.route("**/approve", async (route) => {
      await merging;
      await route.continue();
    });
    await page.clock.install();
    await page.goto(`${hub.url}#/items/app%231`);
    const screen = page.getByRole("region", { name: "app#1" });
    await expect(screen.getByText("Needs review")).toBeVisible();

    await page.mouse.move(10, 10);
    await page.clock.fastForward(10_000);
    await page.mouse.move(20, 20);
    app.seed({ README: "hi there\n" });
    await page.getByRole("button", { name: "Approve & merge" }).click();
    await page.getByRole("button", { name: "Merge", exact: true }).click();
    // The merge takes a minute, with the operator moving about.
    for (const at of [30, 40, 50]) {
      await page.mouse.move(at, at);
      await page.clock.fastForward(20_000);
    }
    merge();
    await expect(screen.getByText("Changes requested")).toBeVisible();
    await page.mouse.move(60, 60);
    await page.clock.fastForward(5_000);
    await page.mouse.move(70, 70);
    await page
      .getByRole("textbox", { name: "Review summary" })
      .fill("Rebase it.");
    await page.getByRole("button", { name: "Request changes" }).click();
    await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();
    hub.mg(["abandon", "app#1"]);

    const [approved, after] = closing(hub, "item.abandoned")?.turns ?? [];
    expect(approved?.verdict).toBe("approved");
    expect(approved?.review_ms).toBeGreaterThanOrEqual(10_000);
    expect(approved?.review_ms).toBeLessThan(20_000);
    expect(after?.verdict).toBe("changes_requested");
    expect(after?.review_ms).toBeGreaterThanOrEqual(5_000);
    expect(after?.review_ms).toBeLessThan(15_000);
  } finally {
    hub.stop();
  }
});
