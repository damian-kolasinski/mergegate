// The review screen warns when no agent will read what the operator sends:
// none waits on the item, and it left no wake command.

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
