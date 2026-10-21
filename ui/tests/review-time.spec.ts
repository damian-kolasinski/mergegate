// Review time: the item screen measures how long the operator is active on
// it, and the merge reports the total for the agent's telemetry, split by
// the turns the operator's reviews end.

import { expect, test } from "./hub";

type Stats = {
  review_ms: number;
  turns: { turn: number; verdict: string | null; review_ms: number }[];
  revisions: { review_ms: number }[];
};

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

  const log = hub.mg(["events", "--since", "0"]) as {
    events: { type: string; stats?: Stats }[];
  };
  const stats = log.events.find((event) => event.type === "item.merged")?.stats;
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
