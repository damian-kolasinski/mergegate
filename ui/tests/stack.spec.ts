// Stacked items from the UI: an item submitted `--on` another reads as its own diff
// in a stack, bottom first, and an approval of it lands right after its
// parent does.

import { expect, test } from "./hub";

test("f16 a stacked item is reviewed on its own and lands after its parent", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  app.stack("profile", "1", "Add profile", {
    "src/profile.rs": "fn profile() {}\n",
  });

  await page.goto(hub.url);
  const needsYou = page.getByRole("region", { name: "Needs you" });
  const rows = needsYou.getByRole("link", { name: /^Add / });
  await expect(rows).toHaveText(["Add login", "Add profile"]);
  await expect(needsYou.getByText("↳ on app#1")).toBeVisible();

  await page.goto(`${hub.url}#/items/app%232`);
  const screen = page.getByRole("region", { name: "app#2" });
  const stack = screen.getByRole("navigation", { name: "Stack" });
  await expect(
    stack.getByRole("link", { name: /app#1 Add login/ }),
  ).toBeVisible();
  await expect(stack.getByText("app#2 Add profile")).toHaveAttribute(
    "aria-current",
    "page",
  );
  // Only its own commits: login's file is app#1's to review.
  await expect(screen.getByText("fn profile() {}")).toBeVisible();
  await expect(screen.getByText("fn login() {}")).toHaveCount(0);

  await page
    .getByRole("button", { name: "Approve, merge after app#1" })
    .click();
  const confirm = page.getByRole("group", { name: "Confirm merge" });
  await expect(confirm.getByText("Stacked on app#1")).toBeVisible();
  await confirm.getByRole("button", { name: "Approve", exact: true }).click();
  const waiting = screen.getByRole("region", { name: "Merge waiting" });
  await expect(waiting.getByText(/Merges right after/)).toBeVisible();

  await stack.getByRole("link", { name: /app#1 Add login/ }).click();
  const parent = page.getByRole("region", { name: "app#1" });
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await confirm.getByRole("button", { name: "Merge" }).click();
  await expect(parent.getByText("Merged", { exact: true })).toBeVisible();

  await page.goto(`${hub.url}#/items/app%232`);
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();
  expect(hub.mg(["feedback", "app#2"])).toMatchObject({
    state: "merged",
    events: [{ type: "review.submitted" }, { type: "item.merged" }],
  });
});

test("f16 a new parent revision leaves its child behind until it rebases", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  app.stack("profile", "1", "Add profile", {
    "src/profile.rs": "fn profile() {}\n",
  });
  app.switch("login");
  app.revise("Take a user", { "src/login.rs": "fn login(user: &str) {}\n" });

  await page.goto(hub.url);
  const needsYou = page.getByRole("region", { name: "Needs you" });
  await expect(needsYou.getByText("↳ behind app#1")).toBeVisible();
  expect(hub.mg(["feedback", "app#2"])).toMatchObject({
    events: [{ type: "parent.updated", parent: "app#1", merged: false }],
  });
});
