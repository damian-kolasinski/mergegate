// F7's last step: the agent answers a comment with `mg reply`, and the open
// item screen shows it on the comment's thread as it happens.

import { expect, test } from "./hub";

test("f7 the agent's replies show on the comment", async ({ page, hub }) => {
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

  const comment = page.getByRole("article", { name: "Comment c_1" });
  const replies = comment.getByRole("list", { name: "Replies" });
  hub.mg(["reply", "c_1", "--status", "question", "--body", "And missing?"]);
  await expect(replies.getByRole("listitem")).toHaveCount(1);
  await expect(replies).toContainText("Question");
  await expect(replies).toContainText("And missing?");

  hub.mg(["reply", "c_1", "--status", "addressed"]);
  await expect(replies.getByRole("listitem")).toHaveCount(2);
  await expect(replies.getByRole("listitem").nth(1)).toHaveText(
    /Agent\s*Addressed in the next revision/,
  );

  // The screen stays on r1, and the agent's r2 is the one that addressed it.
  app.revise("Reject empty passwords", {
    "login.rs": "fn login(password: &str) { assert!(!password.is_empty()) }\n",
  });
  await expect(replies.getByRole("listitem").nth(1)).toContainText(
    "Addressed in r2",
  );
});
