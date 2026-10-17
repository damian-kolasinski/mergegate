// The github backend from the UI: an item's pull request is
// linked from the inbox and the review screen, and opens on GitHub.

import { expect, test } from "./hub";

test("f4 links an item to its pull request", async ({ page, github }) => {
  const app = github.hub.githubProject("app", github.github);
  // So the item's pull request, #84, isn't its ID, app#1.
  await github.github.openElsewhere("acme/app", 83);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  const url = `${github.github.web}/acme/app/pull/84`;
  await page.goto(github.hub.url);

  const row = page
    .getByRole("region", { name: "Needs you" })
    .getByRole("listitem");
  // Named by its pull request, not the hub's own count.
  await expect(row.locator("code")).toHaveText("app#84");
  const link = row.getByRole("link", { name: "View on GitHub" });
  await expect(link).toHaveAttribute("href", url);
  await expect(link).toHaveAttribute("target", "_blank");

  // Following the link opens GitHub, and leaves the inbox where it was.
  const opened = page.waitForEvent("popup");
  await link.click();
  const pull = await opened;
  await expect(pull.getByRole("heading")).toHaveText("Add login #84");
  await expect(pull.getByText("Open", { exact: true })).toBeVisible();
  await expect(page).not.toHaveURL(/#\/items/);

  await row.click();
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByRole("link", { name: "#84" })).toHaveAttribute(
    "href",
    url,
  );
  // The review screen goes by the same name.
  await expect(screen.locator("header code").first()).toHaveText("app#84");
  await expect(page).toHaveTitle("app#84 Add login · mergegate");
  await expect(
    page.getByRole("link", { name: "app#84", exact: true }),
  ).toHaveAttribute("aria-current", "page");
});

test("f8 approving merges the pull request", async ({ page, github }) => {
  const app = github.hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${github.hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page
    .getByRole("group", { name: "Confirm merge" })
    .getByRole("button", { name: "Merge" })
    .click();
  await expect(screen.getByText("Merged", { exact: true })).toBeVisible();

  await page.goto(`${github.github.web}/acme/app/pull/1`);
  await expect(page.getByText(/^Merged as [0-9a-f]{7}$/)).toBeVisible();
});

test("a pull request merged on GitHub merges the item on screen", async ({
  page,
  github,
}) => {
  const app = github.hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${github.hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(
    page.getByRole("button", { name: "Approve & merge" }),
  ).toBeVisible();

  // No reload: the hub's poll finds it and the screen follows.
  await github.github.onGithub(1, "merge");
  await expect(screen.getByRole("region", { name: "Merged" })).toContainText(
    "Merged into main",
  );
  await expect(
    page.getByRole("button", { name: "Approve & merge" }),
  ).toHaveCount(0);

  await page.goto(github.hub.url);
  await expect(page.getByRole("region", { name: "Needs you" })).toContainText(
    "Nothing to review",
  );
  await expect(page.getByText("Closed today (1)")).toBeVisible();
});

test("a pull request closed on GitHub abandons the item", async ({
  page,
  github,
}) => {
  const app = github.hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${github.hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(
    page.getByRole("button", { name: "Approve & merge" }),
  ).toBeVisible();

  await github.github.onGithub(1, "close");
  await expect(screen.getByRole("region", { name: "Abandoned" })).toBeVisible();
});

test("an item on the local backend has no pull request", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(
    page.getByRole("region", { name: "app#1" }).getByText("Add login"),
  ).toBeVisible();
  await expect(page.getByRole("link", { name: /^#\d+/ })).toHaveCount(0);
});
