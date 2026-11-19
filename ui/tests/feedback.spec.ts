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

test("f6 a summary alone hands the item to the agent", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  const review = screen.getByRole("form", { name: "Review" });
  const send = review.getByRole("button", { name: "Send comments" });
  await expect(send).toBeDisabled();
  await expect(
    review.getByRole("button", { name: "Request changes" }),
  ).toHaveCount(0);

  // No line to comment on, as for a rebase: the summary is the ask.
  await page.keyboard.press("r");
  const summary = review.getByRole("textbox", { name: "Review summary" });
  await expect(summary).toBeFocused();
  await page.keyboard.type("Rebase onto main.");
  await send.click();

  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  await expect(summary).toHaveValue("");
  await expect(screen.getByText("Changes requested")).toBeVisible();

  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.state).toBe("changes_requested");
  expect(feedback.events).toMatchObject([
    { revision: 1, verdict: "comment", summary: "Rebase onto main." },
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

test("f6 comments the agent acts on hand the item to it", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("fn login(password: &str) {}")).toBeVisible();

  // `j` puts the cursor on the removed line, then the added one.
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await page
    .getByRole("textbox", { name: "Comment on login.rs line 1" })
    .fill("Check the password.");
  await page.keyboard.press("ControlOrMeta+Enter");
  const review = screen.getByRole("form", { name: "Review" });
  await review.getByRole("button", { name: "Send comments" }).click();

  await expect(review.getByText(/^Sent to the agent on r1/)).toBeVisible();
  await expect(screen.getByText("Changes requested")).toBeVisible();
  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.state).toBe("changes_requested");
  expect(feedback.events).toMatchObject([{ verdict: "comment" }]);
});

test("f6 a newer revision takes the approval away, not comments", async ({
  page,
  hub,
}) => {
  const app = submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByRole("form", { name: "Review" })).toBeVisible();

  app.revise("Log in", { "login.rs": "fn login(password: String) {}\n" });
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner.getByText("r2 submitted")).toBeVisible();

  const review = screen.getByRole("form", { name: "Review" });
  await review
    .getByRole("textbox", { name: "Review summary" })
    .fill("Reject an empty password.");
  const approve = review.getByRole("button", { name: "Approve & merge" });
  await expect(approve).toBeDisabled();
  await expect(approve).toHaveAttribute("title", "Show r2 to approve it");
  // The newest, however many revisions came after the one shown.
  app.revise("Log in again", { "login.rs": "fn login(password: &[u8]) {}\n" });
  await expect(banner.getByText("r3 submitted")).toBeVisible();
  await expect(approve).toHaveAttribute("title", "Show r3 to approve it");

  // A summary on r1 doesn't hand back r3, which the operator hasn't read.
  await review.getByRole("button", { name: "Send comments" }).click();
  await expect(review.getByText("Sent to the agent on r1")).toBeVisible();
  await expect(screen.getByText("Changes requested")).toHaveCount(0);
  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.state).toBe("needs_review");
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

  // Each review hands its item back, and the way on goes to the next one
  // still waiting, until none is.
  const review = page.getByRole("form", { name: "Review" });
  for (const [item, next] of [
    ["app%231", "app#2"],
    ["app%232", "app#3"],
  ] as const) {
    await expect(page).toHaveURL(new RegExp(`#/items/${item}`));
    await review
      .getByRole("textbox", { name: "Review summary" })
      .fill("Name the files.");
    await review.getByRole("button", { name: "Send comments" }).click();
    await expect(review.getByRole("link", { name: /^Next:/ })).toContainText(
      next,
    );
    await page.keyboard.press("Enter");
  }
  await expect(page).toHaveURL(/#\/items\/app%233/);
  await review
    .getByRole("textbox", { name: "Review summary" })
    .fill("Name the files.");
  await review.getByRole("button", { name: "Send comments" }).click();
  await expect(review.getByText("Nothing else needs you")).toBeVisible();
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
