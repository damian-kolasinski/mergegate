// Abandoning from the UI: the operator abandons an item from its header, or sees
// the one its agent abandoned close under them.

import { expect, type Hub, test } from "./hub";

function submitLogin(hub: Hub) {
  const app = hub.project("app");
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
}

test("f14 the operator abandons an item with a reason", async ({
  page,
  hub,
}) => {
  submitLogin(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();
  const more = screen.getByRole("button", { name: "More actions" });
  const open = async () => {
    await more.click();
    await page.getByRole("button", { name: "Abandon…" }).click();
  };

  // Escape backs out of the confirm step without leaving the item.
  await open();
  const confirm = page.getByRole("form", { name: "Abandon" });
  await expect(confirm.getByRole("textbox", { name: "Reason" })).toBeFocused();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(confirm).toHaveCount(0);
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);

  await open();
  await confirm
    .getByRole("textbox", { name: "Reason" })
    .fill("Superseded by app#2.");
  await confirm.getByRole("button", { name: "Abandon app#1" }).click();

  await expect(screen.getByRole("region", { name: "Abandoned" })).toBeVisible();
  await expect(more).toHaveCount(0);
  await expect(page.getByRole("form", { name: "Review" })).toHaveCount(0);
  await expect(
    screen.getByText("Diff dropped when the item closed"),
  ).toBeVisible();

  const feedback = hub.mg(["feedback", "app#1"]) as {
    events: { type: string; source: string; reason: string | null }[];
  };
  expect(feedback.events).toMatchObject([
    {
      type: "item.abandoned",
      source: "operator",
      reason: "Superseded by app#2.",
    },
  ]);
});

test("f14 an item its agent abandons closes on screen", async ({
  page,
  hub,
}) => {
  submitLogin(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("Needs review")).toBeVisible();

  hub.mg(["abandon", "app#1", "--reason", "Moved elsewhere."]);
  await expect(screen.getByRole("region", { name: "Abandoned" })).toBeVisible();
  await expect(page.getByRole("form", { name: "Review" })).toHaveCount(0);
});
