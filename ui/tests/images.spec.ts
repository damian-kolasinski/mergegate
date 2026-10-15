// A changed image opens from its header as both sides and what changed
// between them, and an LFS pointer says what it is.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Locator } from "@playwright/test";
import { expect, type Hub, test } from "./hub";

const fixture = (name: string) =>
  readFileSync(fileURLToPath(new URL(`fixtures/${name}`, import.meta.url)));

/**
 * app#1 changes `art/sunset.png` and adds `art/new.png`, `art/bad.png`,
 * which starts as a PNG and has no header, and `art/broken.png`, whose
 * header is a PNG's and whose pixels aren't.
 */
function submitImages(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "art/sunset.png": fixture("sunset-before.png") });
  app.submit("art", "Warm the sunset", {
    "art/sunset.png": fixture("sunset-after.png"),
    "art/new.png": fixture("sunset-before.png"),
    "art/bad.png": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 1]),
    "art/broken.png": Buffer.concat([png(10, 10), Buffer.from("not pixels")]),
  });
}

/** A PNG's signature and header chunk, claiming `width` × `height`. */
function png(width: number, height: number): Buffer {
  const header = Buffer.alloc(33);
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(header);
  header.writeUInt32BE(13, 8);
  header.write("IHDR", 12, "ascii");
  header.writeUInt32BE(width, 16);
  header.writeUInt32BE(height, 20);
  header.set([8, 6, 0, 0, 0], 24);
  return header;
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
    "Can't show this image: After has no PNG, JPEG, GIF or WebP header.",
  );
  await dialog.getByRole("button", { name: "Close" }).click();

  await section(diff, "art/broken.png")
    .getByRole("button", { name: "Show image" })
    .click();
  await expect(dialog).toContainText(
    "Can't show this image: the browser can't decode it.",
  );
});

test("every format the preview reads decodes at its size", async ({
  page,
  hub,
}) => {
  const formats = [
    "sunset.jpg",
    "sunset.gif",
    "sunset-lossy.webp",
    "sunset-lossless.webp",
    "sunset-alpha.webp",
  ];
  hub
    .project("app")
    .submit(
      "art",
      "Add the sunset in every format",
      Object.fromEntries(formats.map((name) => [`art/${name}`, fixture(name)])),
    );
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  const dialog = page.getByRole("dialog");
  for (const name of formats) {
    await section(diff, `art/${name}`)
      .getByRole("button", { name: "Show image" })
      .click();
    await expect(dialog.getByRole("img", { name: "After" })).toBeVisible();
    await expect(dialog).toContainText("240 × 160");
    await dialog.getByRole("button", { name: "Close" }).click();
    await expect(dialog).toBeHidden();
  }
});

test("an image claiming too many pixels isn't decoded", async ({
  page,
  hub,
}) => {
  // A few bytes each, every one stating 20000 × 20000: 1.6 GB decoded.
  const side = 20_000;
  const jpeg = Buffer.alloc(21);
  jpeg.set([0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08]);
  jpeg.writeUInt16BE(side, 7);
  jpeg.writeUInt16BE(side, 9);
  const gif = Buffer.alloc(13);
  gif.write("GIF89a", 0, "ascii");
  gif.writeUInt16LE(side, 6);
  gif.writeUInt16LE(side, 8);
  const webp = Buffer.alloc(30);
  webp.write("RIFF", 0, "ascii");
  webp.writeUInt32LE(22, 4);
  webp.write("WEBPVP8X", 8, "ascii");
  webp.writeUInt32LE(10, 16);
  webp.writeUIntLE(side - 1, 24, 3);
  webp.writeUIntLE(side - 1, 27, 3);
  const huge = {
    "art/huge.png": png(side, side),
    "art/huge.jpg": jpeg,
    "art/huge.gif": gif,
    "art/huge.webp": webp,
  };
  hub.project("app").submit("art", "Add huge art", huge);
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  const dialog = page.getByRole("dialog");
  for (const path of Object.keys(huge)) {
    await section(diff, path)
      .getByRole("button", { name: "Show image" })
      .click();
    await expect(dialog).toContainText(
      "Can't show this image: After is 20000 × 20000 pixels, past the 16-megapixel limit.",
    );
    await dialog.getByRole("button", { name: "Close" }).click();
    await expect(dialog).toBeHidden();
  }
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
