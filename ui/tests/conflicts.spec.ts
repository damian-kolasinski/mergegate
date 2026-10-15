// F9 before review: the inbox flags open items that conflict with each
// other and puts them first, and an item whose target moved under it goes
// back to its agent without the operator doing anything.

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

test("f9 items that conflict with each other are flagged and first", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.submit("greet", "Greet", { README: "hello\n" });
    app.submit("docs", "Write the guide", { "guide.md": "Guide\n" });
    app.submit("wave", "Wave", { README: "hey\n" });
    await page.goto(hub.url);

    const rows = region(page, "Needs you");
    await expect(rows).toHaveText([/app#1/, /app#3/, /app#2/]);
    await expect(rows.nth(0)).toContainText("⚡ conflicts with app#3");
    await expect(rows.nth(1)).toContainText("⚡ conflicts with app#1");
    await expect(rows.nth(2)).not.toContainText("conflicts with");
  });
});

test("f9 an open item's page follows the items it conflicts with", async ({
  page,
}) => {
  await watching(async (hub) => {
    const app = hub.project("app");
    app.submit("greet", "Greet", { README: "hello\n" });
    await page.goto(`${hub.url}#/items/app%231`);
    const screen = page.getByRole("region", { name: "app#1" });
    await expect(screen.getByText("Needs review")).toBeVisible();
    await page.keyboard.press("a");
    const confirm = page.getByRole("group", { name: "Confirm merge" });
    await expect(confirm).toBeVisible();
    await expect(confirm).not.toContainText("Conflicts with");

    app.submit("wave", "Wave", { README: "hey\n" });
    await expect(confirm).toContainText("Conflicts with app#2");

    app.revise("Wave elsewhere", { README: null, "wave.txt": "hey\n" });
    await expect(confirm).not.toContainText("Conflicts with");
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
    await expect(back.getByText("Conflict", { exact: true })).toBeVisible();
    const feedback = hub.mg(["feedback", "app#1"]) as {
      events: { type: string; paths: string[] }[];
    };
    expect(feedback.events).toMatchObject([
      { type: "merge.conflict", paths: ["README"] },
    ]);
  });
});
