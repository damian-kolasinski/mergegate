// F6 and F7 end to end: the operator comments on lines and requests changes,
// the agent waits for it and answers, the operator answers back, and once the
// agent's next revision is in, r1's comments follow their lines into it.

import { expect, test } from "./hub";

type Wait = {
  events: {
    revision: number;
    verdict: string;
    comments: { id: string; revision: number; quote: string; body: string }[];
    replies: { comment: string; body: string }[];
  }[];
};

test("f6 f7 a review goes round between the operator and the agent", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {\n    check(password);\n}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("check(password);")).toBeVisible();
  const header = page.getByRole("region", { name: "app#1" });
  const review = page.getByRole("form", { name: "Review" });

  // The operator comments on a line and requests changes.
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Reject an empty password.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(review.getByText("1 draft")).toBeVisible();
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText("Sent to the agent on r1 with 1 comment"),
  ).toBeVisible();

  // The agent's wait returns it, quoting the line, and the agent asks back.
  const asked = hub.mg(["wait", "app#1", "--for", "operator"]) as Wait;
  expect(asked.events).toMatchObject([
    {
      revision: 1,
      verdict: "changes_requested",
      comments: [
        {
          id: "c_1",
          quote: "fn login(password: &str) {",
          body: "Reject an empty password.",
        },
      ],
    },
  ]);
  hub.mg(["reply", "c_1", "--status", "question", "--body", "And missing?"]);
  const comment = diff.getByRole("article", { name: "Comment c_1" });
  const replies = comment.getByRole("list", { name: "Replies" });
  await expect(replies).toContainText("And missing?");

  // The operator answers, and drafts one more comment it doesn't send yet.
  await comment.getByRole("button", { name: "Reply" }).click();
  await expect(
    comment.getByRole("textbox", { name: "Reply to c_1" }),
  ).toBeFocused();
  await page.keyboard.type("Both.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(review.getByText("1 draft reply")).toBeVisible();
  await review.getByRole("button", { name: "Comment" }).click();
  await expect(
    review.getByText("Sent to the agent on r1 with 1 reply"),
  ).toBeVisible();
  const answered = hub.mg(["wait", "app#1", "--for", "operator"]) as Wait;
  expect(answered.events).toMatchObject([
    { verdict: "comment", replies: [{ comment: "c_1", body: "Both." }] },
  ]);
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 2" }),
  ).toBeFocused();
  await page.keyboard.type("Log the failure.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(
    header.getByRole("button", { name: "Conversation 2" }),
  ).toBeVisible();

  // The agent addresses c_1 in r2.
  hub.mg(["reply", "c_1", "--status", "addressed"]);
  app.revise("Reject empty passwords", {
    "login.rs":
      "fn login(password: &str) {\n    assert!(!password.is_empty());\n    check(password);\n}\n",
  });

  // Open on r2, r1's comments sit on their unchanged lines, marked with the
  // revision they were written on; the list keeps them as written.
  await page.reload();
  await expect(diff.getByText("assert!(!password.is_empty());")).toBeVisible();
  await expect(header.getByText("r2", { exact: true })).toBeVisible();
  await expect(diff.getByRole("article")).toHaveCount(2);
  await expect(
    diff.getByRole("article", { name: "Comment c_1" }),
  ).toContainText("from r1");
  const moved = diff.getByRole("article", { name: "Comment c_2" });
  await expect(moved).toContainText("from r1");
  const toggle = header.getByRole("button", { name: "Conversation 2" });
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  const list = page.getByRole("complementary", { name: "Conversation" });
  const sent = list.getByRole("listitem", { name: "c_1" });
  await expect(sent).toContainText("login.rs:1 @r1");
  await expect(sent.getByText("Addressed in r2")).toBeVisible();
  // The diff holds the thread; the list sums it up.
  await expect(sent).toContainText("3 replies");
  const draft = list.getByRole("listitem", { name: "c_2" });
  await expect(draft).toContainText("login.rs:2 @r1");
  await expect(draft.getByText("Draft")).toBeVisible();

  // r1's draft goes out with the review written on r2, on r2's lines.
  await toggle.click();
  await expect(list).toHaveCount(0);
  await moved.getByRole("button", { name: "Edit" }).click();
  await diff
    .getByRole("textbox", { name: "Edit comment c_2" })
    .fill("Log the failure, please.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(moved).toContainText("Log the failure, please.");
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText("Sent to the agent on r2 with 1 comment"),
  ).toBeVisible();
  await toggle.click();
  await expect(draft.getByText("Sent")).toBeVisible();
  await expect(draft).toContainText("login.rs:3 @r2");
  const third = hub.mg(["wait", "app#1"]) as Wait;
  expect(third.events).toMatchObject([
    {
      revision: 2,
      verdict: "changes_requested",
      comments: [
        {
          id: "c_2",
          revision: 2,
          quote: "    check(password);",
          body: "Log the failure, please.",
        },
      ],
    },
  ]);

  // Escape closes the list before it leaves the item.
  await page.keyboard.press("Escape");
  await expect(list).toHaveCount(0);
  await expect(toggle).toBeFocused();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await expect(header).toBeVisible();
});

test("show in diff closes the list on the comment's line", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const header = page.getByRole("region", { name: "app#1" });
  await header.getByRole("button", { name: "Conversation 0" }).click();
  const list = page.getByRole("complementary", { name: "Conversation" });
  await expect(list).toContainText("No comments yet");
  await page.keyboard.press("Escape");
  await expect(list).toHaveCount(0);

  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Why?");
  await page.keyboard.press("ControlOrMeta+Enter");
  await header.getByRole("button", { name: "Conversation 1" }).click();
  const entry = list.getByRole("listitem", { name: "c_1" });
  await expect(entry).toContainText("login.rs:1 @r1");
  await expect(entry).toContainText("Why?");
  // The diff's own revision keeps the thread under its line.
  await expect(entry.getByRole("article")).toHaveCount(0);
  await entry.getByRole("button", { name: "Show in diff" }).click();
  await expect(list).toHaveCount(0);
  await expect(
    page.getByRole("article", { name: "Comment c_1" }),
  ).toBeInViewport();
});

test("show in diff opens a viewed file at the comment", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const header = page.getByRole("region", { name: "app#1" });
  await expect(page.getByText("fn login(password: &str) {}")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Why?");
  await page.keyboard.press("ControlOrMeta+Enter");
  const comment = page.getByRole("article", { name: "Comment c_1" });
  await expect(comment).toBeVisible();
  const mark = page
    .getByRole("region", { name: "Diff" })
    .getByLabel("Viewed login.rs");
  await mark.check();
  await expect(comment).toHaveCount(0);

  await header.getByRole("button", { name: "Conversation 1" }).click();
  const list = page.getByRole("complementary", { name: "Conversation" });
  await list.getByRole("button", { name: "Show in diff" }).click();
  await expect(list).toHaveCount(0);
  await expect(comment).toBeInViewport();
  // Opened, not unmarked: marking it again collapses it.
  await expect(mark).toBeChecked();
  await mark.uncheck();
  await mark.check();
  await expect(comment).toHaveCount(0);
});
