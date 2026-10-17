// Priorities in the UI: the inbox shows each item's priority and note and orders
// by them, and the operator changes them with `p` there or `P` on the item.

import { expect, type Hub, test } from "./hub";

/** `app#1` to `app#3`, oldest first, with `app#3` at p1. */
function three(hub: Hub) {
  const app = hub.project("app");
  app.submit("parser", "Add the parser", { "parser.rs": "p\n" });
  app.submit("lexer", "Add the lexer", { "lexer.rs": "l\n" });
  app.submit("hotfix", "Fix the crash", { "fix.rs": "f\n" });
  hub.mg(["prioritize", "app#3", "--priority", "p1", "--note", "users hit it"]);
}

function rows(page: import("@playwright/test").Page) {
  return page.getByRole("region", { name: "Needs you" }).getByRole("listitem");
}

test("f10 the inbox orders by priority and shows the note", async ({
  page,
  hub,
}) => {
  three(hub);
  await page.goto(hub.url);
  await expect(rows(page)).toHaveText([/app#3/, /app#1/, /app#2/]);
  const first = rows(page).first();
  await expect(first).toContainText("p1");
  await expect(first).toContainText("users hit it");

  // The operator's agent reprioritizing moves the row without a reload.
  hub.mg(["prioritize", "app#2", "--priority", "p0"]);
  await expect(rows(page)).toHaveText([/app#2/, /app#3/, /app#1/]);
});

test("f10 p sets the selected item's priority and note", async ({
  page,
  hub,
}) => {
  three(hub);
  await page.goto(hub.url);
  const selected = page.locator("li[aria-current=true]");
  await expect(selected).toContainText("app#3");
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await expect(selected).toContainText("app#2");

  await page.keyboard.press("p");
  const picker = page.getByRole("dialog", { name: "Priority of app#2" });
  await expect(picker).toBeVisible();
  // The picker has the keys: j leaves the selection where it is.
  await page.keyboard.press("j");
  await page.keyboard.press("0");
  await expect(picker.getByRole("radio", { name: "p0" })).toBeChecked();
  await picker.getByRole("textbox", { name: "Note" }).fill("the demo");
  await picker.getByRole("button", { name: "Save" }).click();

  await expect(picker).toHaveCount(0);
  await expect(rows(page)).toHaveText([/app#2/, /app#3/, /app#1/]);
  await expect(rows(page).first()).toContainText("the demo");
  const inbox = hub.mg(["inbox"]) as {
    needs_you: { id: string; priority: string; note: string }[];
  };
  expect(inbox.needs_you[0]).toMatchObject({
    id: "app#2",
    priority: "p0",
    note: "the demo",
  });

  // Escape backs out without a change; n clears the priority.
  await page.keyboard.press("p");
  await page.keyboard.press("n");
  await page.keyboard.press("Escape");
  await expect(picker).toHaveCount(0);
  await expect(rows(page).first()).toContainText("p0");
  await page.keyboard.press("p");
  await page.keyboard.press("n");
  await page.keyboard.press("Enter");
  await expect(picker).toHaveCount(0);
  await expect(rows(page)).toHaveText([/app#3/, /app#1/, /app#2/]);
  await expect(page).toHaveURL(hub.url);
});

test("f10 shift-p sets the priority from the item's header", async ({
  page,
  hub,
}) => {
  three(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const header = page.getByRole("region", { name: "app#1" }).locator("header");
  await expect(header).toContainText("Needs review");
  await expect(header).not.toContainText("p2");

  await page.keyboard.press("P");
  const picker = page.getByRole("dialog", { name: "Priority of app#1" });
  await page.keyboard.press("2");
  await page.keyboard.press("Enter");
  await expect(picker).toHaveCount(0);
  await expect(header).toContainText("p2");

  // The screen's keys stand down under it: `n` is no priority, not the
  // next file, and Escape closes the picker without leaving the item.
  await page.keyboard.press("P");
  await page.keyboard.press("n");
  await expect(picker.getByRole("radio", { name: "none" })).toBeChecked();
  await page.keyboard.press("Escape");
  await expect(picker).toHaveCount(0);
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);
  await expect(header).toContainText("p2");

  // What the operator's agent sets shows without a reopen, and the picker
  // opens on it rather than on what the screen loaded with.
  hub.mg(["prioritize", "app#1", "--priority", "p3", "--note", "later"]);
  await expect(header).toContainText("p3");
  await page.keyboard.press("P");
  await expect(picker.getByRole("radio", { name: "p3" })).toBeChecked();
  await expect(picker.getByRole("textbox", { name: "Note" })).toHaveValue(
    "later",
  );
  // Enter on a focused Cancel cancels.
  await picker.getByRole("button", { name: "Cancel" }).focus();
  await page.keyboard.press("2");
  await page.keyboard.press("Enter");
  await expect(picker).toHaveCount(0);
  await expect(header).toContainText("p3");
});

test("f10 the picker over the merge confirm closes first", async ({
  page,
  hub,
}) => {
  three(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await page.keyboard.press("a");
  const confirm = page.getByRole("group", { name: "Confirm merge" });
  await expect(confirm).toBeVisible();
  await page.keyboard.press("P");
  const picker = page.getByRole("dialog", { name: "Priority of app#1" });
  await expect(picker).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(picker).toHaveCount(0);
  await expect(confirm).toBeVisible();
});
