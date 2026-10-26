// Conflicts before review: an item whose target moved under it goes back to its
// agent without the operator doing anything, and the inbox and its page say
// it conflicts with the target. Open items aren't merged into each other.

import type { Page } from "@playwright/test";
import { expect, Hub, test } from "./hub";

/** A hub that test-merges often, so a moved target shows up quickly. */
async function watching(run: (hub: Hub) => Promise<void>) {
  const hub = new Hub({ MG_TARGET_POLL_MS: "200" });
  try {
    await run(hub);
  } finally {
    hub.stop();
  }
}

function region(page: Page, name: string) {
  return page.getByRole("region", { name }).getByRole("listitem");
}

test("f9 items that conflict only with each other aren't flagged", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.submit("greet", "Greet", { README: "hello\n" });
    app.submit("wave", "Wave", { README: "hey\n" });
    await page.goto(hub.url);
    const needsYou = region(page, "Needs you");
    await expect(needsYou).toHaveText([/app#1/, /app#2/]);
    // Passes enough for the watch to have tried both.
    await page.waitForTimeout(1000);
    await expect(page.getByText("Conflicts with")).toHaveCount(0);

    // Landing one makes the other conflict with the target.
    app.seed({ README: "hello\n" });
    const back = region(page, "With agents");
    await expect(back).toHaveText([/app#2/]);
    await expect(back.getByText("Conflicts with main")).toBeVisible();
    await expect(needsYou).toHaveText([/app#1/]);
  });
});

test("f9 an open item's page shows the conflict the watch finds", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.seed({ README: "hello\n" });
    app.submit("readme", "Greet", { README: "hello, login\n" });
    await page.goto(`${hub.url}#/items/app%231`);
    const screen = page.getByRole("region", { name: "app#1" });
    await expect(screen.getByText("Needs review")).toBeVisible();

    app.seed({ README: "hi there\n" });
    const conflict = screen.getByRole("region", { name: "Merge conflict" });
    await expect(conflict.getByText("README", { exact: true })).toBeVisible();
    await expect(conflict).toContainText("Conflicts with main");
    await expect(screen.getByText("Changes requested")).toBeVisible();
  });
});

test("f9 a target moved under an item sends it back before review", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.seed({ README: "hello\n" });
    app.submit("readme", "Greet", { README: "hello, login\n" });
    await page.goto(hub.url);
    await expect(region(page, "Needs you")).toHaveText([/app#1/]);

    app.seed({ README: "hi there\n" });
    const back = region(page, "With agents");
    await expect(back).toHaveText([/app#1/]);
    await expect(back.getByText("Conflicts with main")).toBeVisible();
    const feedback = hub.mg(["feedback", "app#1"]) as {
      events: { type: string; paths: string[] }[];
    };
    expect(feedback.events).toMatchObject([
      { type: "merge.conflict", paths: ["README"] },
    ]);
  });
});

test("f9 an open item's page says whether it merges cleanly", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.seed({ README: "hello\n" });
    app.submit("readme", "Greet", { README: "hello, login\n" });
    await page.goto(`${hub.url}#/items/app%231`);
    const screen = page.getByRole("region", { name: "app#1" });
    await expect(screen.getByText("Merges cleanly")).toBeVisible();

    // The merge-blocked note says the conflict the watch hands back.
    app.seed({ README: "hi there\n" });
    const conflict = screen.getByRole("region", { name: "Merge conflict" });
    await expect(conflict).toContainText("Conflicts with main");
    await expect(screen.getByText("Merges cleanly")).toHaveCount(0);
  });
});
