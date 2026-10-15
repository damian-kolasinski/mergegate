// F6: the operator drafts comments on lines of the diff, and the review sends
// them to the agent, which reads them with their lines (F7).

import { expect, type Hub, test } from "./hub";

function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n", "notes.md": "one\ntwo\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
    "notes.md": "one\ntwo\nthree\n",
  });
}

type Feedback = {
  events: {
    verdict: string;
    summary: string;
    comments: { id: string; path: string; side: string; line: number }[];
  }[];
};

test("f6 drafts written on the diff go out with the review", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("fn login(password: &str) {}")).toBeVisible();

  // `j` puts the cursor on the removed line, then the added one.
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  const editor = page.getByRole("textbox", {
    name: "Comment on login.rs line 1",
  });
  await expect(editor).toBeFocused();
  await page.keyboard.type("Check the password.");
  await page.keyboard.press("ControlOrMeta+Enter");

  const first = page.getByRole("article", { name: "Comment c_1" });
  await expect(first).toContainText("Draft");
  await expect(first).toContainText("Check the password.");
  await expect(editor).toHaveCount(0);

  await first.getByRole("button", { name: "Edit" }).click();
  const edit = page.getByRole("textbox", { name: "Edit comment c_1" });
  await expect(edit).toBeFocused();
  await edit.fill("Reject an empty password.");
  await page.getByRole("button", { name: "Save draft" }).click();
  await expect(first).toContainText("Reject an empty password.");

  // Outside the cursor's file, the gutter's `+` comments on the line under
  // the pointer.
  const notes = diff.getByRole("code").filter({ hasText: "three" });
  await notes.getByText("three", { exact: true }).hover();
  await notes.locator("[data-utility-button]").click();
  await page
    .getByRole("textbox", { name: "Comment on notes.md line 3" })
    .fill("Say why.");
  await page.getByRole("button", { name: "Save draft" }).click();
  const second = page.getByRole("article", { name: "Comment c_2" });
  await expect(second).toContainText("Say why.");
  await second.getByRole("button", { name: "Delete" }).click();
  await expect(second).toHaveCount(0);

  const review = page.getByRole("form", { name: "Review" });
  await expect(review.getByText("1 draft comment")).toBeVisible();
  // A draft is enough to send without a summary.
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText("Sent to the agent on r1 with 1 comment"),
  ).toBeVisible();
  await expect(first).toContainText("Sent");
  await expect(first.getByRole("button", { name: "Edit" })).toHaveCount(0);
  await expect(review.getByText("1 draft comment")).toHaveCount(0);

  const feedback = hub.mg(["feedback", "app#1"]) as Feedback;
  expect(feedback.events).toMatchObject([
    {
      verdict: "changes_requested",
      summary: "",
      comments: [{ id: "c_1", path: "login.rs", side: "new", line: 1 }],
    },
  ]);
});

test("drafts stay on the hub until sent", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.getByText("three", { exact: true })).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on login.rs line 1" }),
  ).toBeFocused();
  await page.keyboard.type("On the old line.");
  await page.getByRole("button", { name: "Save draft" }).click();
  await expect(
    page.getByRole("article", { name: "Comment c_1" }),
  ).toBeVisible();

  await page.reload();
  const draft = page.getByRole("article", { name: "Comment c_1" });
  await expect(draft).toContainText("On the old line.");
  await expect(draft).toContainText("Draft");
  await expect(page.getByText("1 draft comment")).toBeVisible();

  // An empty comment closes on Escape and leaves nothing behind.
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: /^Comment on/ }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("textbox", { name: /^Comment on/ })).toHaveCount(
    0,
  );
});

test("an open comment keeps what's typed in it", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("three", { exact: true })).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  // In the cursor's file, `+` comments on the cursor's line.
  const login = diff.getByRole("code").filter({ hasText: "fn login" });
  await login.locator("[data-utility-button]").click();
  const editor = page.getByRole("textbox", {
    name: "Comment on login.rs line 1",
  });
  await expect(editor).toBeFocused();
  await page.keyboard.type("Half a thought");

  // Escape leaves the field, and again brings it back instead of leaving.
  await page.keyboard.press("Escape");
  await expect(editor).not.toBeFocused();
  await page.keyboard.press("Escape");
  await expect(editor).toBeFocused();
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);

  // Another line's `+` and the shortcuts leave it be.
  const notes = diff.getByRole("code").filter({ hasText: "three" });
  await notes.getByText("three", { exact: true }).hover();
  await notes.locator("[data-utility-button]").click();
  await expect(editor).toHaveValue("Half a thought");
  await expect(editor).toBeFocused();

  // The review waits for it.
  const review = page.getByRole("form", { name: "Review" });
  await review.getByRole("textbox", { name: "Review summary" }).fill("Done.");
  await expect(
    review.getByText("Save or cancel the open comment first"),
  ).toBeVisible();
  await expect(
    review.getByRole("button", { name: "Request changes" }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Cancel" }).click();
  await expect(editor).toHaveCount(0);
  await expect(
    review.getByRole("button", { name: "Request changes" }),
  ).toBeEnabled();
});

test("a file marked viewed stays open while a comment is written on it", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("fn login(password: &str) {}")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  const editor = page.getByRole("textbox", {
    name: "Comment on login.rs line 1",
  });
  await expect(editor).toBeFocused();
  await page.keyboard.type("Half a thought");
  await page
    .getByRole("navigation", { name: "Files" })
    .getByLabel("Viewed login.rs")
    .check();
  await expect(page.getByText("1 of 2 reviewed")).toBeVisible();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveValue("Half a thought");

  // Saved, the file collapses with its comment.
  await editor.focus();
  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(editor).toHaveCount(0);
  await expect(diff.getByText("fn login(password: &str) {}")).toHaveCount(0);
  const review = page.getByRole("form", { name: "Review" });
  await expect(
    review.getByRole("button", { name: "Request changes" }),
  ).toBeEnabled();
});
