// The agent answers a comment with `mg reply`, and the open
// item screen shows it on the comment's thread as it happens. The operator
// answers back with a draft reply, which the next review sends.

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

test("f7 the operator's reply goes out with the next review", async ({
  page,
  hub,
}) => {
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
  await expect(
    review.getByText("Sent to the agent on r1 with 1 comment"),
  ).toBeVisible();
  hub.mg(["reply", "c_1", "--status", "question", "--body", "And missing?"]);

  const comment = page.getByRole("article", { name: "Comment c_1" });
  const replies = comment.getByRole("list", { name: "Replies" });
  await expect(replies.getByRole("listitem")).toHaveCount(1);
  await comment.getByRole("button", { name: "Reply" }).click();
  await expect(
    comment.getByRole("textbox", { name: "Reply to c_1" }),
  ).toBeFocused();
  await page.keyboard.type("Both.");
  await page.keyboard.press("ControlOrMeta+Enter");
  const draft = replies.getByRole("listitem").nth(1);
  await expect(draft).toHaveText(/You\s*Draft.*Both\./);
  await expect(review.getByText("1 draft reply")).toBeVisible();

  await draft.getByRole("button", { name: "Edit" }).click();
  const field = comment.getByRole("textbox", { name: "Edit reply to c_1" });
  await field.fill("Both, please.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(draft).toContainText("Both, please.");

  // The agent answering meanwhile leaves the draft as it is.
  hub.mg(["reply", "c_1", "--status", "question", "--body", "Even spaces?"]);
  await expect(replies.getByRole("listitem")).toHaveCount(3);
  await expect(draft).toHaveText(/You\s*Draft.*Both, please\./);
  await comment.getByRole("button", { name: "Reply" }).click();
  await page.keyboard.type("Spaces too.");
  await page.keyboard.press("ControlOrMeta+Enter");
  const extra = replies.getByRole("listitem").nth(3);
  await expect(extra).toContainText("Spaces too.");
  await expect(review.getByText("2 draft replies")).toBeVisible();
  await extra.getByRole("button", { name: "Delete" }).click();
  await expect(replies.getByRole("listitem")).toHaveCount(3);
  await expect(review.getByText("1 draft reply")).toBeVisible();

  // A draft reply alone is enough to send a review.
  await review.getByRole("button", { name: "Comment" }).click();
  await expect(
    review.getByText("Sent to the agent on r1 with 1 reply"),
  ).toBeVisible();
  await expect(draft.getByText("Draft")).toHaveCount(0);
  await expect(draft.getByRole("button", { name: "Edit" })).toHaveCount(0);
  const feedback = hub.mg(["feedback", "app#1", "--since", "1"]) as {
    events: { replies: { comment: string; body: string }[] }[];
  };
  expect(feedback.events[0]?.replies).toMatchObject([
    { comment: "c_1", body: "Both, please." },
  ]);
});
