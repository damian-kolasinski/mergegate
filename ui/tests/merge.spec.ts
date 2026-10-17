// F8 and F9 from the UI: the operator approves an item and it merges, or
// sees why it couldn't: a conflict sent back to the agent, or a merge held
// until they retry or cancel it.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, Hub, test } from "./hub";

function events(hub: Hub): { type: string }[] {
  return (hub.mg(["feedback", "app#1"]) as { events: { type: string }[] })
    .events;
}

test("f8 approving merges the item", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  // `a` asks first, and Escape backs out without leaving the item.
  await page.keyboard.press("a");
  const confirm = page.getByRole("group", { name: "Confirm merge" });
  await expect(confirm.getByText("1 file not reviewed yet")).toBeVisible();
  await expect(confirm.getByRole("button", { name: "Merge" })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(confirm).toHaveCount(0);
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);

  await page.getByRole("textbox", { name: "Review summary" }).fill("Ship it.");
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await confirm.getByRole("button", { name: "Merge" }).click();

  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();
  await expect(page.getByRole("form", { name: "Review" })).toHaveCount(0);
  // A merged item keeps its diff.
  await expect(screen.getByText("fn login() {}")).toBeVisible();
  expect(events(hub)).toMatchObject([
    { type: "review.submitted", verdict: "approved", summary: "Ship it." },
    { type: "item.merged" },
  ]);
});

test("f9 a conflict shows what to rebase", async ({ page }) => {
  // Without the target watch, which would find the moved target first
  // (conflicts.spec.ts).
  const hub = new Hub({ MG_TARGET_WATCH: "off" });
  try {
    const app = hub.project("app");
    app.seed({ README: "hello\n" });
    app.submit("readme", "Greet", { README: "hello, login\n" });
    app.seed({ README: "hi there\n" });
    await page.goto(`${hub.url}#/items/app%231`);
    const screen = page.getByRole("region", { name: "app#1" });
    await page.getByRole("button", { name: "Approve & merge" }).click();
    await page.getByRole("button", { name: "Merge", exact: true }).click();

    const conflict = screen.getByRole("region", { name: "Merge conflict" });
    await expect(conflict.getByText("README", { exact: true })).toBeVisible();
    await expect(conflict.getByText("Sent back to the agent")).toBeVisible();
    await expect(screen.getByText("Changes requested")).toBeVisible();
    expect(events(hub).map((event) => event.type)).toEqual([
      "review.submitted",
      "merge.conflict",
    ]);

    await page.keyboard.press("Escape");
    const withAgents = page.getByRole("region", { name: "With agents" });
    await expect(
      withAgents.getByRole("listitem").getByText("Conflicts with main"),
    ).toBeVisible();
  } finally {
    hub.stop();
  }
});

test("f8 a held merge waits for a retry or a cancel", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ README: "hello\n" });
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  // The operator is on `main` with an edit, which the push can't update.
  app.seed({ NOTES: "notes\n" });
  writeFileSync(join(app.dir, "README"), "edited\n");
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();

  const held = screen.getByRole("region", { name: "Merge held" });
  await expect(held).toContainText("refused main");
  await expect(screen.getByText("Merging", { exact: true })).toBeVisible();
  await expect(page.getByRole("form", { name: "Review" })).toHaveCount(0);

  // Still in the way, so it holds again.
  await held.getByRole("button", { name: "Retry merge" }).click();
  await expect
    .poll(() => events(hub).filter((e) => e.type === "merge.held").length)
    .toBe(2);
  await expect(held).toContainText("refused main");

  writeFileSync(join(app.dir, "README"), "hello\n");
  await held.getByRole("button", { name: "Retry merge" }).click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();
  await expect(held).toHaveCount(0);
});

test("f8 cancelling a held merge puts the item back in review", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ README: "hello\n" });
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  app.seed({ NOTES: "notes\n" });
  writeFileSync(join(app.dir, "README"), "edited\n");
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  const held = screen.getByRole("region", { name: "Merge held" });
  await held.getByRole("button", { name: "Cancel merge" }).click();

  await expect(screen.getByText("Needs review")).toBeVisible();
  await expect(held).toHaveCount(0);
  await expect(page.getByRole("form", { name: "Review" })).toBeVisible();
  expect(events(hub).map((event) => event.type)).toEqual([
    "review.submitted",
    "merge.held",
    "merge.cancelled",
  ]);
});

test("f8 the merge confirm waits for an open comment", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("fn login() {}")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("a");
  const merge = page
    .getByRole("group", { name: "Confirm merge" })
    .getByRole("button", { name: "Merge" });
  await expect(merge).toBeFocused();

  await page.keyboard.press("c");
  const editor = page.getByRole("textbox", {
    name: "Comment on src/login.rs line 1",
  });
  await expect(editor).toBeFocused();
  await page.keyboard.type("Name the error.");
  await expect(merge).toBeDisabled();

  // Saved, the comment goes out with the merge.
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(editor).toHaveCount(0);
  await merge.click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();
  expect(events(hub)).toMatchObject([
    {
      type: "review.submitted",
      verdict: "approved",
      comments: [{ body: "Name the error." }],
    },
    { type: "item.merged" },
  ]);
});

test("f8 a newer revision takes the merge away from the confirm", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("fn login() {}")).toBeVisible();
  await page.keyboard.press("a");
  const merge = page
    .getByRole("group", { name: "Confirm merge" })
    .getByRole("button", { name: "Merge" });
  await expect(merge).toBeFocused();

  app.revise("Log in", { "src/login.rs": "fn login() { todo!() }\n" });
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner.getByText("r2 submitted")).toBeVisible();
  await expect(merge).toBeDisabled();
});

test("f8 a held merge cancelled after a reload takes comments again", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ README: "hello\n" });
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  app.seed({ NOTES: "notes\n" });
  writeFileSync(join(app.dir, "README"), "edited\n");
  // Held before the screen opens.
  const approved = await page.request.post(
    `${hub.url}api/items/app%231/approve`,
    {
      data: { revision: 1, summary: "" },
      headers: { "x-mergegate-client": "ui" },
    },
  );
  expect(approved.ok()).toBe(true);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const held = screen.getByRole("region", { name: "Merge held" });
  await held.getByRole("button", { name: "Cancel merge" }).click();
  await expect(screen.getByText("Needs review")).toBeVisible();

  const line = screen.getByRole("code").filter({ hasText: "fn login" });
  await line.getByText("fn login() {}").hover();
  await expect(line.locator("[data-utility-button]")).toHaveCount(1);

  // Held again, the gutter stops offering it.
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(held).toBeVisible();
  await line.getByText("fn login() {}").hover();
  await expect(line.locator("[data-utility-button]")).toHaveCount(0);
});
