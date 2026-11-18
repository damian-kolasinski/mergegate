// The review screen's header says only what tells the operator something:
// no state while the item needs review, no reviewed count until a file is
// marked, and the branch in a tooltip rather than on the line.

import { expect, test } from "./hub";

test("the header leaves out what the usual review doesn't need", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n", "Cargo.lock": "a\n" });
  await hub.rules(
    "app",
    `[[rule]]\nid = "R1"\ntier = "trusted"\npath = "**/*.lock"\nreason = "generated"\n`,
  );
  app.submit("login", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
    "notes.md": "one\n",
    "Cargo.lock": "b\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const header = page
    .getByRole("region", { name: "app#1" })
    .locator("header")
    .first();
  await expect(
    header.getByRole("heading", { name: "Add login" }),
  ).toBeVisible();

  await expect(header).not.toContainText("Needs review");
  await expect(header).not.toContainText("login → main");
  await expect(header.locator('[title="login → main"]')).toHaveText("app#1");
  await expect(header).toContainText("1 trusted");
  await expect(header).not.toContainText("reviewed");

  await page
    .getByRole("region", { name: "Diff" })
    .getByLabel("Viewed login.rs")
    .check();
  await expect(header).toContainText("1 of 2 reviewed · 1 trusted");
});
