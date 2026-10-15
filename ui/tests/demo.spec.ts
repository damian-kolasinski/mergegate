// The review-screen part of F16: what an agent attaches shows as a chip in
// the item's header, opens full size, and marks its row in the inbox.

import { fileURLToPath } from "node:url";
import { expect, test } from "./hub";

const screen = fileURLToPath(new URL("fixtures/screen.png", import.meta.url));
const flow = fileURLToPath(new URL("fixtures/flow.webm", import.meta.url));

test("f16 attachments show in the header and open full size", async ({
  page,
  hub,
}) => {
  hub
    .project("app")
    .submit("parser", "Add the parser", { "p.rs": "p\n" }, [screen, flow]);
  await page.goto(`${hub.url}#/items/app%231`);

  const chip = page.getByRole("button", { name: /^Demo \(/ });
  await expect(chip).toHaveText("▶Demo (2)");

  await chip.click();
  const viewer = page.getByRole("dialog");
  await expect(viewer).toHaveAccessibleName("screen.png");
  await expect(viewer.getByRole("img", { name: "screen.png" })).toBeVisible();
  await expect(viewer).toContainText("1 / 2");

  await page.keyboard.press("ArrowRight");
  await expect(viewer).toHaveAccessibleName("flow.webm");
  await expect(viewer.locator("video")).toBeVisible();
  await page.keyboard.press("ArrowLeft");
  await expect(viewer).toHaveAccessibleName("screen.png");

  // Escape closes the viewer, not the review.
  await page.keyboard.press("Escape");
  await expect(viewer).toBeHidden();
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);
  await expect(chip).toBeFocused();
});

test("f16 a revision without attachments shows the last ones, labeled", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("parser", "Add the parser", { "p.rs": "p\n" }, [screen]);
  app.revise("Fix a typo", { "p.rs": "q\n" });
  await page.goto(`${hub.url}#/items/app%231`);

  const chip = page.getByRole("button", { name: /^Demo \(/ });
  await expect(chip).toHaveText("▶Demo (1)from r1");
});

test("f16 an attach shows without a reload", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("parser", "Add the parser", { "p.rs": "p\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.getByRole("region", { name: "app#1" })).toBeVisible();
  const chip = page.getByRole("button", { name: /^Demo \(/ });
  await expect(chip).toHaveCount(0);

  hub.mg(["attach", "app#1", flow], app.dir);
  await chip.click();
  await expect(page.getByRole("dialog")).toHaveAccessibleName("flow.webm");
});

test("f16 the inbox marks items with a demo", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("plain", "No demo", { "a.txt": "a\n" });
  app.submit("shown", "With a demo", { "b.txt": "b\n" }, [screen]);
  await page.goto(hub.url);

  const rows = page.getByRole("listitem");
  await expect(rows.filter({ hasText: "With a demo" })).toContainText("demo");
  await expect(rows.filter({ hasText: "No demo" })).not.toContainText("▶");
});
