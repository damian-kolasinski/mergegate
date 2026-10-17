// The operator's review goes to the agent, which reads it
// with `mg feedback`.

import { expect, git, type Hub, test } from "./hub";

function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  return app;
}

type Feedback = {
  state: string;
  events: { revision: number; verdict: string; summary: string }[];
};

test("f6 request changes sends the summary to the agent", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();
  const review = screen.getByRole("form", { name: "Review" });
  const request = review.getByRole("button", { name: "Request changes" });
  await expect(request).toBeDisabled();

  await page.keyboard.press("r");
  const summary = review.getByRole("textbox", { name: "Review summary" });
  await expect(summary).toBeFocused();
  await page.keyboard.type("Reject an empty password.");
  await request.click();

  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  await expect(summary).toHaveValue("");
  await expect(screen.getByText("Changes requested")).toBeVisible();

  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.state).toBe("changes_requested");
  expect(feedback.events).toMatchObject([
    {
      revision: 1,
      verdict: "changes_requested",
      summary: "Reject an empty password.",
    },
  ]);
});

test("f6 ⌘↩ sends comments from outside the summary", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const review = page.getByRole("form", { name: "Review" });
  const summary = review.getByRole("textbox", { name: "Review summary" });
  // Nothing written, nothing to send.
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(review.getByText(/^Sent to the agent/)).toHaveCount(0);

  await summary.fill("Looks close.");
  await summary.blur();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  await expect(summary).toHaveValue("");
  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.events).toMatchObject([
    { revision: 1, verdict: "comment", summary: "Looks close." },
  ]);
});

test("f6 comment leaves the item with the operator", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const review = page.getByRole("form", { name: "Review" });
  await review
    .getByRole("textbox", { name: "Review summary" })
    .fill("Looking good so far.");
  await review.getByRole("button", { name: "Comment" }).click();

  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  await expect(page.getByText("Needs review")).toBeVisible();
  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.events).toMatchObject([{ verdict: "comment" }]);
});

test("f6 a newer revision takes request changes away, not comments", async ({
  page,
  hub,
}) => {
  const app = submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  app.revise("Log in", { "login.rs": "fn login(password: String) {}\n" });
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner.getByText("r2 submitted")).toBeVisible();

  // The hub turns away changes requested on code the agent already replaced.
  const review = screen.getByRole("form", { name: "Review" });
  await review
    .getByRole("textbox", { name: "Review summary" })
    .fill("Reject an empty password.");
  const request = review.getByRole("button", { name: "Request changes" });
  await expect(request).toBeDisabled();
  await expect(request).toHaveAttribute("title", "Show r2 to request changes");
  // The newest, however many revisions came after the one shown.
  app.revise("Log in again", { "login.rs": "fn login(password: &[u8]) {}\n" });
  await expect(banner.getByText("r3 submitted")).toBeVisible();
  await expect(request).toHaveAttribute("title", "Show r3 to request changes");
  await expect(
    review.getByRole("button", { name: "Approve & merge" }),
  ).toHaveAttribute("title", "Show r3 to approve it");
  await review.getByRole("button", { name: "Send comments" }).click();
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
});

test("f6 the way on after a review reaches every item that needs the operator", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("a", "First", { "a.txt": "a\n" });
  app.submit("b", "Second", { "b.txt": "b\n" });
  app.submit("c", "Third", { "c.txt": "c\n" });
  await page.goto(`${hub.url}#/items/app%231`);

  // A comment leaves each item in Needs you, where the way on goes past it
  // rather than back to it, round after round.
  const review = page.getByRole("form", { name: "Review" });
  for (const [item, next] of [
    ["app%231", "app#2"],
    ["app%232", "app#3"],
    ["app%233", "app#1"],
    ["app%231", "app#2"],
    ["app%232", "app#3"],
  ] as const) {
    await expect(page).toHaveURL(new RegExp(`#/items/${item}`));
    await review
      .getByRole("textbox", { name: "Review summary" })
      .fill("Looking good so far.");
    await review.getByRole("button", { name: "Send comments" }).click();
    await expect(review.getByRole("link", { name: /^Next:/ })).toContainText(
      next,
    );
    await page.keyboard.press("Enter");
  }
  await expect(page).toHaveURL(/#\/items\/app%233/);
});

test("f6 an item revised after a review comes next again", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("a", "First", { "a.txt": "a\n" });
  app.submit("b", "Second", { "b.txt": "b\n" });
  app.submit("c", "Third", { "c.txt": "c\n" });
  await page.goto(`${hub.url}#/items/app%231`);

  const review = page.getByRole("form", { name: "Review" });
  const next = review.getByRole("link", { name: /^Next:/ });
  await review.getByRole("textbox", { name: "Review summary" }).fill("Hm.");
  await review.getByRole("button", { name: "Send comments" }).click();
  await expect(next).toContainText("app#2");

  // The agent answers with r2, which puts app#1 first in line again.
  git(app.dir, "switch", "--quiet", "a");
  app.revise("First, again", { "a.txt": "a2\n" });
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/items\/app%232/);
  await review.getByRole("textbox", { name: "Review summary" }).fill("Hm.");
  await review.getByRole("button", { name: "Send comments" }).click();
  await expect(next).toContainText("app#1");
});
