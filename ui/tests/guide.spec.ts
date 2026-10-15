// The guide part of F11: the agent's guide shows as the item's intent, and
// its `## Files` list orders the files to review and says why each matters.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "./hub";

const GUIDE = `Splits parsing out of main, so the CLI can reuse it.

## Files

- \`src/parser.rs\` — the new parser, start here
- \`src/main.rs\` — now only calls it
`;

test("f11 the guide is the intent and orders the review", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("parser", "Split out the parser", {
    "README.md": "# app\n",
    "src/main.rs": "fn main() { parse() }\n",
    "src/parser.rs": "fn parse() {}\n",
  });
  // At the same head, it only sets the guide.
  const guide = join(hub.root, "guide.md");
  writeFileSync(guide, GUIDE);
  hub.mg(["submit", "--guide", guide], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  const fold = screen.locator("summary").filter({ hasText: "Intent" });
  // Folded to its first line, so the diff keeps the screen.
  await expect(fold).toContainText(
    "Splits parsing out of main, so the CLI can reuse it.",
  );
  const intent = screen.getByRole("region", { name: "Intent" });
  await expect(intent).toBeHidden();
  await fold.click();
  // The file list says what the Files section did, so it isn't repeated.
  await expect(intent).toHaveText(
    "Splits parsing out of main, so the CLI can reuse it.",
  );

  const files = screen.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^parser\.rs\s*src\s*the new parser, start here\s*\+1$/,
    /^main\.rs\s*src\s*now only calls it\s*\+1$/,
    /^README\.md\s*ranked low: docs\s*\+1$/,
  ]);
  const current = files.locator("li[aria-current=true]");
  await expect(current).toContainText("parser.rs");
  await page.keyboard.press("n");
  await expect(current).toContainText("main.rs");

  // Folded away again, it leaves the diff to the files.
  await fold.click();
  await expect(intent).toBeHidden();
});

test("f11 a long reason gives way to the file's name in its header", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("parser", "Split out the parser", {
    "src/parser.rs": "fn parse() {}\n",
  });
  const guide = join(hub.root, "guide.md");
  const reason = `${"the new parser, ".repeat(20)}start here`;
  writeFileSync(
    guide,
    `Splits parsing out.\n\n## Files\n\n- \`src/parser.rs\` — ${reason}\n`,
  );
  hub.mg(["submit", "--guide", guide], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  const header = page.locator("[data-diffs-header]").first();
  const title = header.locator("[data-title]");
  await expect(title).toHaveText("src/parser.rs");
  // Whole, and the note cut short inside the header rather than past it.
  expect(await title.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
    true,
  );
  // Slotted into the header from the page, so not inside its shadow root.
  const note = page.locator('[slot="header-metadata"] > span').first();
  await expect(note).toContainText(reason);
  const box = await header.boundingBox();
  const noteBox = await note.boundingBox();
  expect(box && noteBox && noteBox.x + noteBox.width <= box.x + box.width).toBe(
    true,
  );
  expect(await note.evaluate((el) => el.scrollWidth > el.clientWidth)).toBe(
    true,
  );
});

test("f11 without a guide there is no intent", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("copy", "Fix the copy", { "README.md": "# app\n" });
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByRole("navigation", { name: "Files" })).toBeVisible();
  await expect(screen.getByRole("region", { name: "Intent" })).toHaveCount(0);
});

test("f11 a list in the intent keeps its bullets inside", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("copy", "Fix the copy", { "README.md": "# app\n" });
  const guide = join(hub.root, "guide.md");
  writeFileSync(guide, "Fixes the copy:\n\n- the title\n- the intro\n");
  hub.mg(["submit", "--guide", guide], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  await screen.locator("summary").filter({ hasText: "Intent" }).click();
  const list = screen.getByRole("region", { name: "Intent" }).getByRole("list");
  await expect(list.getByRole("listitem")).toHaveText([
    "the title",
    "the intro",
  ]);
  // The bullets hang in the padding, past the text's edge without it.
  await expect(list).not.toHaveCSS("padding-left", "0px");
});
