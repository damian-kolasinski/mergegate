// F6's last step: the operator's review goes to the agent, which reads it
// with `mg feedback` (F7).

import { expect, type Hub, test } from "./hub";

function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
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
