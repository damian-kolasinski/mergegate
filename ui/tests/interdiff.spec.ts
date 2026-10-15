// F7 from the UI: a revision submitted mid-review waits behind a banner, and
// switching to it shows only what changed since the revision reviewed, over
// a target that moved meanwhile.

import type { Page } from "@playwright/test";
import { expect, git, type Hub, test } from "./hub";

/**
 * app#1 r1 adds `login.rs` and changes `lib.rs`; the operator views
 * `login.rs` and requests changes. Returns a step that has the agent rebase
 * onto a `main` whose README moved and change `lib.rs` again, as r2.
 */
function reviewed(hub: Hub) {
  const app = hub.project("app");
  app.seed({ README: "title\n", "src/lib.rs": "lib\n" });
  app.submit("login", "Add login", {
    "src/lib.rs": "lib\nmod login;\n",
    "src/login.rs": "fn login() {}\n",
  });
  return () => {
    app.seed({ README: "title\nmore\n" });
    git(app.dir, "switch", "--quiet", "login");
    git(app.dir, "rebase", "--quiet", "main");
    app.revise("Add tokens", { "src/lib.rs": "lib\nmod login;\nmod token;\n" });
  };
}

async function requestChanges(page: Page) {
  const review = page.getByRole("form", { name: "Review" });
  await review.getByRole("textbox", { name: "Review summary" }).fill("Fix it.");
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
}

test("f7 a new revision shows what changed since the review", async ({
  page,
  hub,
}) => {
  const rebase = reviewed(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const files = screen.getByRole("navigation", { name: "Files" });
  const diff = screen.getByRole("region", { name: "Diff" });
  const review = page.getByRole("form", { name: "Review" });
  const current = files.locator("li[aria-current=true]");
  // It adds a declaration, which ranks it first.
  await expect(current).toContainText("src/login.rs");
  await files.getByLabel("Viewed src/login.rs").check();
  await requestChanges(page);

  // r2 waits behind a banner, and only it can be approved.
  rebase();
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner.getByText("r2 submitted")).toBeVisible();
  await expect(diff.getByText("mod token;")).toHaveCount(0);
  await expect(
    review.getByRole("button", { name: "Approve & merge" }),
  ).toBeDisabled();

  // What's typed survives the switch and comparing another way.
  const summary = review.getByRole("textbox", { name: "Review summary" });
  await summary.fill("Closer.");
  await banner.getByRole("button", { name: "Show changes since r1" }).click();
  await expect(banner).toHaveCount(0);
  await expect(summary).toHaveValue("Closer.");
  await expect(screen.getByLabel("Compare")).toHaveValue("1");
  // main's README isn't the item's, and login.rs is as r1 left it, its
  // mark with it.
  await expect(files.getByRole("listitem")).toHaveText([/^src\/lib\.rs/]);
  // login.rs was current, and isn't shown now.
  await expect(current).toContainText("src/lib.rs");
  await expect(diff.getByText("mod token;")).toBeVisible();
  await expect(diff.getByText("more", { exact: true })).toHaveCount(0);
  await files.getByText("1 unchanged since r1").click();
  await expect(files.getByLabel("Viewed src/login.rs")).toBeChecked();
  await expect(screen.getByText("1/2 viewed")).toBeVisible();
  await expect(
    review.getByRole("button", { name: "Approve & merge" }),
  ).toBeEnabled();

  await screen.getByLabel("Compare").selectOption("all");
  await expect(files.getByRole("listitem")).toHaveText([
    /^src\/login\.rs/,
    /^src\/lib\.rs/,
  ]);
  await expect(files.getByText("unchanged since")).toHaveCount(0);
  await expect(summary).toHaveValue("Closer.");
  // lib.rs is shown again with its changes against the base: `mod login;`
  // is r1's addition, context since r1.
  const added = diff.locator('[data-line-type="change-addition"]');
  await expect(added.filter({ hasText: /^mod login;$/ })).toHaveCount(1);
});

test("f7 the inbox counts what changed since the review", async ({
  page,
  hub,
}) => {
  const rebase = reviewed(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await requestChanges(page);
  rebase();

  await page.goto(hub.url);
  await expect(page.getByRole("listitem")).toContainText("1 changed since r1");

  // Opening it starts there.
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("Compare")).toHaveValue("1");
});
