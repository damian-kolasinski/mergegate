// The operator folds comment threads to a line, one at a time or all at
// once, as GitHub folds a conversation, and the tab remembers them.

import { expect, test } from "./hub";

test("a thread folds to a line and back", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("fn login(password: &str) {}")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Reject an empty password.");
  await page.keyboard.press("ControlOrMeta+Enter");
  const review = page.getByRole("form", { name: "Review" });
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  hub.mg(["reply", "c_1", "--status", "question", "--body", "And missing?"]);

  const comment = page.getByRole("article", { name: "Comment c_1" });
  const replies = comment.getByRole("list", { name: "Replies" });
  await expect(replies).toContainText("And missing?");

  // Folded, the replies go and the agent's answer moves to the header.
  await comment.getByRole("button", { name: "Collapse comment c_1" }).click();
  await expect(replies).toBeHidden();
  const excerpt = comment.getByRole("button", {
    name: /^Reject an empty password\.\s·\s1 reply$/,
  });
  await expect(excerpt).toBeVisible();
  await expect(comment.locator("header")).toContainText("Question");

  // The tab remembers it.
  await page.reload();
  await expect(excerpt).toBeVisible();
  await expect(replies).toBeHidden();

  // Every thread is folded, so the header's button and `z` unfold them.
  const all = page.getByRole("button", { name: "Expand comments" });
  await expect(all).toBeVisible();
  await page.keyboard.press("z");
  await expect(replies).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Collapse comments" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Collapse comments" }).click();
  await expect(replies).toBeHidden();

  // Clicking the line unfolds it.
  await excerpt.click();
  await expect(replies).toBeVisible();

  // A reply opened on a folded thread unfolds it, so what's typed shows.
  await comment.getByRole("button", { name: "Collapse comment c_1" }).click();
  await comment.getByRole("button", { name: "Reply", exact: true }).click();
  const reply = comment.getByRole("textbox", { name: "Reply to c_1" });
  await expect(reply).toBeFocused();
  await expect(replies).toBeVisible();

  // Nor does it fold while the reply is open, which would hide the editor
  // that holds the review.
  await expect(
    comment.getByRole("button", { name: "Collapse comment c_1" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Collapse comments" }).click();
  await expect(reply).toBeVisible();
});

test("the header's controls share one height", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("fn login(password: &str) {}")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Reject an empty password.");
  await page.keyboard.press("ControlOrMeta+Enter");
  const review = page.getByRole("form", { name: "Review" });
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();

  // Collapse comments' key hint once made it taller than the rest.
  // The first header is the item's; comments have their own.
  const header = page
    .getByRole("region", { name: "app#1" })
    .locator("header")
    .first();
  await expect(
    header.getByRole("button", { name: "Collapse comments" }),
  ).toBeVisible();
  const heights = await header
    .locator("button:visible, select:visible")
    .evaluateAll((controls) =>
      controls.map((control) => control.getBoundingClientRect().height),
    );
  expect(heights.length).toBeGreaterThan(2);
  expect(new Set(heights)).toEqual(new Set([32]));
});
