// A changed image opens from its header as both sides and what changed
// between them, and an LFS pointer says what it is.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Locator } from "@playwright/test";
import { expect, type Hub, test } from "./hub";

const fixture = (name: string) =>
  readFileSync(fileURLToPath(new URL(`fixtures/${name}`, import.meta.url)));

/**
 * app#1 changes `art/sunset.png`, adds `art/new.png`, and adds `art/bad.png`,
 * which says it's a PNG and isn't.
 */
function submitImages(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "art/sunset.png": fixture("sunset-before.png") });
  app.submit("art", "Warm the sunset", {
    "art/sunset.png": fixture("sunset-after.png"),
    "art/new.png": fixture("sunset-before.png"),
    "art/bad.png": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 1]),
  });
}

test("a changed image shows both sides and the pixels that changed", async ({
  page,
  hub,
}) => {
  submitImages(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("Binary file not shown")).toHaveCount(0);

  await section(diff, "art/sunset.png")
    .getByRole("button", { name: "Compare images" })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAccessibleName("art/sunset.png");
  for (const side of ["Before", "After", "Changes"]) {
    const pane = dialog.getByRole("img", { name: side });
    await expect(pane).toBeVisible();
    // Painted, not only sized: the sunset is opaque everywhere.
    const alpha = await pane.evaluate((canvas: HTMLCanvasElement) => {
      const context = canvas.getContext("2d");
      return context?.getImageData(10, 10, 1, 1).data[3];
    });
    expect(alpha).toBeGreaterThan(0);
  }
  await expect(dialog).toContainText("240 × 160");
  await expect(dialog).toContainText("1955 of 38400 pixels changed (5.09%)");

  // Escape closes the dialog, not the review.
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page).toHaveURL(/#\/items\/app%231(\?.*)?$/);
});

test("an added image shows alone, and one that won't decode says so", async ({
  page,
  hub,
}) => {
  submitImages(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });

  await section(diff, "art/new.png")
    .getByRole("button", { name: "Show image" })
    .click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAccessibleName("art/new.png");
  await expect(dialog.getByRole("img", { name: "After" })).toBeVisible();
  await expect(dialog.getByRole("img")).toHaveCount(1);
  await dialog.getByRole("button", { name: "Close" }).click();
  await expect(dialog).toBeHidden();

  await section(diff, "art/bad.png")
    .getByRole("button", { name: "Show image" })
    .click();
  await expect(dialog).toContainText(
    "Can't show this image: the browser can't decode it.",
  );
});

test("an LFS pointer says the hub doesn't hold its file", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("lfs", "Track the video", {
    "intro.mp4": [
      "version https://git-lfs.github.com/spec/v1",
      "oid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393",
      "size 12345",
      "",
    ].join("\n"),
  });
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(
    page
      .getByRole("region", { name: "Diff" })
      .getByText("Git LFS pointer; the hub doesn't hold the file it points to"),
  ).toBeVisible();
});

/** The diff's section for `path`, whose name CodeView keeps in a shadow root. */
function section(diff: Locator, path: string): Locator {
  return diff
    .locator("diffs-container")
    .filter({ has: diff.page().getByText(path, { exact: true }) });
}
