// An approval follows the diff: a revision that changes nothing since the
// approved one lands on its own, and one that does says how far it moved
// from what the operator approved.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, git, type Hub, test } from "./hub";

/**
 * `app#1` approved at r1 and held by an edit in the operator's checkout of
 * `main`, which then moved; the edit is gone and `login` is rebased onto it.
 */
async function heldThenRebased(page: Page, hub: Hub) {
  const app = hub.project("app");
  app.seed({ README: "hello\n" });
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  app.seed({ NOTES: "notes\n" });
  writeFileSync(join(app.dir, "README"), "edited\n");
  await page.goto(`${hub.url}#/items/app%231`);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(
    screen.getByRole("region", { name: "Merge held" }),
  ).toBeVisible();
  writeFileSync(join(app.dir, "README"), "hello\n");
  git(app.dir, "switch", "--quiet", "login");
  git(app.dir, "rebase", "--quiet", "main");
  return { app, screen };
}

test("f8 an approval carries to a revision with the same diff", async ({
  page,
  hub,
}) => {
  const { app, screen } = await heldThenRebased(page, hub);
  hub.mg(["submit"], app.dir);
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();
  await expect(screen.getByText("r2", { exact: true })).toBeVisible();
});

test("f8 a revision that moved on from the approval says so", async ({
  page,
  hub,
}) => {
  const { app } = await heldThenRebased(page, hub);
  app.revise("Take a user", { "src/login.rs": "fn login(user: &str) {}\n" });

  await page.goto(hub.url);
  const needsYou = page.getByRole("region", { name: "Needs you" });
  await expect(needsYou.getByText("1 changed since approved r1")).toBeVisible();
  await page.goto(`${hub.url}#/items/app%231`);
  const review = page.getByRole("form", { name: "Review" });
  await expect(
    review.getByText("Approved r1 · 1 file changed since"),
  ).toBeVisible();
});
