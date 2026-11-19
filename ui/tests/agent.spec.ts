// The review screen warns when no agent will read what the operator sends:
// none waits on the item, and it left no wake command.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, type Hub, test } from "./hub";

function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  return app;
}

test("an item no agent waits on warns that feedback stays here", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const away = page.getByRole("region", { name: "No agent" });
  await expect(away).toBeVisible();
  await expect(away.getByText("mg resume app#1")).toBeVisible();

  // An agent's wait clears it as it starts, without a reload.
  hub.wait("app#1");
  await expect(away).toHaveCount(0);
});

test("an item with a wake command doesn't warn", async ({ page, hub }) => {
  const app = submitChange(hub);
  // A submit at the same commit adds no revision, but records the wake.
  hub.mg(["submit", "--wake", "true"], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  await expect(page.getByRole("region", { name: "app#1" })).toBeVisible();
  await expect(page.getByText("Add login").first()).toBeVisible();
  await expect(page.getByRole("region", { name: "No agent" })).toHaveCount(0);
});

test("a closed item doesn't warn", async ({ page, hub }) => {
  submitChange(hub);
  hub.mg(["abandon", "app#1", "--reason", "Not needed"]);
  await page.goto(`${hub.url}#/items/app%231`);

  await expect(page.getByText("Abandoned").first()).toBeVisible();
  await expect(page.getByRole("region", { name: "No agent" })).toHaveCount(0);
});

test("a merging item warns above the diff, with no review bar to hold it", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ README: "hello\n" });
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  // An edit on the checked-out target holds the merge, which stays Merging.
  app.seed({ NOTES: "notes\n" });
  writeFileSync(join(app.dir, "README"), "edited\n");
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await page.getByRole("button", { name: "Approve & merge" }).click();

  await expect(
    screen.getByRole("region", { name: "Merge held" }),
  ).toBeVisible();
  await expect(page.getByRole("form", { name: "Review" })).toHaveCount(0);
  const away = screen.getByRole("region", { name: "No agent" });
  await expect(away.getByText("mg resume app#1")).toBeVisible();
});
