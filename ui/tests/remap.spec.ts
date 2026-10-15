// F7 and F13 from the UI: once the agent's next revision is in, comments on
// lines it kept or changed follow them into its diff, those on lines it
// deleted stay in the Conversation list, and drafts that can't move hold the
// review until the operator re-places or deletes them.

import type { Page } from "@playwright/test";
import type { WriteDraft } from "../api/WriteDraft";
import { expect, test } from "./hub";

type Wait = {
  events: {
    revision: number;
    comments: { id: string; revision: number; line: number; quote: string }[];
  }[];
};

const r1 = [
  "fn login(password: &str) {",
  "    check(password);",
  "    audit();",
  "    log();",
  "    store();",
  "    done();",
  "    save();",
  "    trace();",
  "}",
];

// Lines 2 and 6 change, 4 and 8 go, the rest stay.
const r2 = [
  "fn login(password: &str) {",
  "    verify(password);",
  "    audit();",
  "    store();",
  "    finish();",
  "    save();",
  "}",
];

// What r2's diff against r1 shows, top to bottom: each change's removed lines
// come before the lines that replaced them.
const since = [
  "fn login(password: &str) {",
  "check(password);",
  "verify(password);",
  "audit();",
  "log();",
  "store();",
  "done();",
  "finish();",
  "save();",
  "trace();",
  "}",
];

const file = (lines: string[]) => `${lines.join("\n")}\n`;

/** Walks the keyboard cursor to the line with this text in `shown`. */
async function cursorTo(page: Page, shown: string[], text: string) {
  const cursor = page.locator("[data-line][data-editor-active-line]");
  if (!(await cursor.count())) await page.keyboard.press("j");
  const from = shown.indexOf((await cursor.innerText()).trim());
  const to = shown.indexOf(text);
  for (let at = from; at !== to; at += Math.sign(to - from)) {
    await page.keyboard.press(to > from ? "j" : "k");
  }
  await expect(cursor).toHaveText(text);
}

async function draft(page: Page, line: number, body: string) {
  const shown = r1.map((text) => text.trim());
  await cursorTo(page, shown, shown[line - 1] ?? "");
  await page.keyboard.press("c");
  const editor = page.getByRole("textbox", {
    name: `Comment on login.rs line ${line}`,
  });
  await editor.fill(body);
  await page.keyboard.press("ControlOrMeta+Enter");
  // The cursor's keys go to the diff once the editor is gone.
  await expect(editor).toHaveCount(0);
}

test("f13 comments follow their lines to the next revision", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "README.md": "app\n" });
  app.submit("work", "Add login", { "login.rs": file(r1) });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const diff = screen.getByRole("region", { name: "Diff" });
  const review = page.getByRole("form", { name: "Review" });

  // r1 is reviewed, and more is drafted on it before r2 comes in.
  await draft(page, 2, "Say what check does.");
  await draft(page, 4, "Log less.");
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText("Sent to the agent on r1 with 2 comments"),
  ).toBeVisible();
  hub.mg(["wait", "app#1", "--for", "operator"]);
  await draft(page, 3, "Audit failures too.");
  await draft(page, 4, "Or drop it.");
  await draft(page, 6, "Rename it.");
  await draft(page, 8, "Trace at debug.");
  app.revise("Verify passwords", { "login.rs": file(r2) });
  await screen
    .getByRole("region", { name: "Newer revision" })
    .getByRole("button", { name: "Show changes since r1" })
    .click();
  await expect(diff.getByText("verify(password);")).toBeVisible();

  // A sent comment follows its changed line, a draft its unchanged one.
  const changed = diff.getByRole("article", { name: "Comment c_1" });
  await expect(changed.getByText("Changed since r1")).toBeVisible();
  await expect(changed).toContainText("Line 2");
  const moved = diff.getByRole("article", { name: "Comment c_3" });
  await expect(moved).toContainText("from r1");
  await expect(moved).toContainText("Line 3");
  await expect(moved.getByRole("button", { name: "Re-place" })).toHaveCount(0);

  // The drafts whose lines changed or went hold the review.
  await expect(
    review.getByText("3 drafts can't move to r2 — re-place or delete them"),
  ).toBeVisible();
  for (const name of ["Comment", "Request changes", "Approve & merge"]) {
    await expect(review.getByRole("button", { name })).toBeDisabled();
  }

  // One on a changed line shows there until it's kept or re-placed.
  const renamed = diff.getByRole("article", { name: "Comment c_5" });
  await expect(renamed.getByText("Changed since r1")).toBeVisible();
  await renamed.getByRole("button", { name: "Keep here" }).click();
  await expect(renamed.getByText("Changed since r1")).toHaveCount(0);
  await expect(renamed).toContainText("Line 5");
  await expect(
    review.getByText("2 drafts can't move to r2 — re-place or delete them"),
  ).toBeVisible();

  // Those on deleted lines are only in the list, beside the sent one.
  const toggle = screen.getByRole("button", { name: "Conversation 6" });
  await toggle.click();
  const list = page.getByRole("complementary", { name: "Conversation" });
  const gone = list.getByRole("listitem", { name: "c_2" });
  await expect(gone).toContainText("login.rs:4 @r1");
  await expect(gone.getByText("Outdated")).toBeVisible();
  await expect(gone.getByRole("button", { name: "Show in diff" })).toHaveCount(
    0,
  );
  await expect(diff.getByRole("article", { name: "Comment c_2" })).toHaveCount(
    0,
  );

  // Re-placing closes the list for the diff's lines; Escape backs out.
  const traced = list.getByRole("listitem", { name: "c_6" });
  await traced.getByRole("button", { name: "Re-place" }).click();
  await expect(list).toHaveCount(0);
  await expect(
    review.getByText("Pick the lines for c_6 in the diff"),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(review.getByText("2 drafts can't move to r2")).toBeVisible();
  await toggle.click();
  await traced.getByRole("button", { name: "Re-place" }).click();
  await cursorTo(page, since, "save();");
  await page.keyboard.press("c");
  const placed = diff.getByRole("article", { name: "Comment c_6" });
  await expect(placed).toContainText("Trace at debug.");
  await expect(placed).toContainText("Line 6");

  // The last one goes.
  await toggle.click();
  await list
    .getByRole("listitem", { name: "c_4" })
    .getByRole("button", { name: "Delete" })
    .click();
  await expect(list.getByRole("listitem", { name: "c_4" })).toHaveCount(0);
  await expect(review.getByText("can't move to r2")).toHaveCount(0);

  // The review sends every draft on r2's lines.
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText("Sent to the agent on r2 with 3 comments"),
  ).toBeVisible();
  const sent = hub.mg(["wait", "app#1", "--for", "operator"]) as Wait;
  expect(sent.events).toMatchObject([
    {
      revision: 2,
      comments: [
        { id: "c_3", revision: 2, line: 3, quote: "    audit();" },
        { id: "c_5", revision: 2, line: 5, quote: "    finish();" },
        { id: "c_6", revision: 2, line: 6, quote: "    save();" },
      ],
    },
  ]);
});

test("drafts from another tab that hold the review can be dealt with", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "README.md": "app\n" });
  app.submit("work", "Add login", { "login.rs": file(r1) });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const diff = screen.getByRole("region", { name: "Diff" });
  const review = page.getByRole("form", { name: "Review" });
  app.revise("Verify passwords", { "login.rs": file(r2) });
  await screen
    .getByRole("region", { name: "Newer revision" })
    .getByRole("button", { name: "Show changes since r1" })
    .click();
  await expect(diff.getByText("verify(password);")).toBeVisible();

  // Another tab, still on r1, drafts on lines r2 deleted and changed.
  for (const [line, body] of [
    [4, "Log less."],
    [6, "Rename it."],
  ] as const) {
    const draft: WriteDraft = {
      revision: 1,
      path: "login.rs",
      side: "new",
      start_line: line,
      line,
      body,
    };
    const answer = await page.request.post(
      `${hub.url}api/items/app%231/drafts`,
      { data: draft, headers: { "x-mergegate-client": "ui" } },
    );
    expect(answer.ok(), await answer.text()).toBe(true);
  }

  // The hub holds the review, and this tab picks the drafts up.
  await review.getByRole("textbox", { name: "Review summary" }).fill("Again.");
  await review.getByRole("button", { name: "Comment" }).click();
  await expect(
    review.getByText("2 drafts can't move to r2 — re-place or delete them"),
  ).toBeVisible();

  // Deleting the draft being re-placed ends the re-placing.
  const renamed = diff.getByRole("article", { name: "Comment c_2" });
  await renamed.getByRole("button", { name: "Re-place" }).click();
  await expect(review.getByText("Pick the lines for c_2")).toBeVisible();
  await renamed.getByRole("button", { name: "Delete" }).click();
  await expect(renamed).toHaveCount(0);
  await expect(review.getByText("Pick the lines")).toHaveCount(0);
  await expect(
    review.getByText("1 draft can't move to r2 — re-place or delete it"),
  ).toBeVisible();

  await screen.getByRole("button", { name: /^Conversation/ }).click();
  await page
    .getByRole("complementary", { name: "Conversation" })
    .getByRole("listitem", { name: "c_1" })
    .getByRole("button", { name: "Delete" })
    .click();
  await review.getByRole("button", { name: "Comment" }).click();
  await expect(review.getByText("Sent to the agent on r2")).toBeVisible();
});
