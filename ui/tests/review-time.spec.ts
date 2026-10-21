// Review time: the item screen measures how long the operator is active on
// it, and the merge reports the total for the agent's telemetry, split by
// the turns the operator's reviews end.

import { expect, type Hub, test } from "./hub";

type Stats = {
  review_ms: number;
  turns: { turn: number; verdict: string | null; review_ms: number }[];
  revisions: { revision: number; review_ms: number }[];
};

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
  // The first report reaches the hub only after the review would have.
  let held = false;
  await page.route("**/review-time", async (route) => {
    if (!held) {
      held = true;
      await new Promise((resolve) => setTimeout(resolve, 1_000));
    }
    await route.continue();
  });
  await page.clock.install();
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  await page.mouse.move(10, 10);
  await page.clock.fastForward(20_000);
  await page.mouse.move(20, 20);
  // The half-minute report sends the twenty seconds, and is held.
  await page.clock.fastForward(10_000);
  await page
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looks close.");
  await page.getByRole("button", { name: "Send comments" }).click();
  await expect(screen.getByText("Sent to the agent on r1")).toBeVisible();
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  const stats = closing(hub, "item.merged");
  expect(held).toBe(true);
  expect(stats?.turns[0]?.review_ms).toBeGreaterThanOrEqual(20_000);
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
