// A changed Markdown file opens from its header as it reads after the change,
// built from elements, so the file can't run or load anything.

import { expect, test } from "./hub";

const doc = [
  "# Setup",
  "",
  "Run `mg hub start` **first**, then *read* the [guide](https://example.com/guide).",
  "See [the notes](docs/notes.md) and keep snake_case_names as written.",
  "",
  "## Steps",
  "",
  "1. Install",
  "2. Configure",
  "   - set `PORT`",
  "   - set `HOME`",
  "",
  "> Back up the database first.",
  "",
  "```sh",
  "mg hub start --port 0",
  "```",
  "",
  "| Flag | Meaning |",
  "| --- | --- |",
  "| `--json` | machine output |",
  "",
  "![the inbox](https://example.com/inbox.png)",
  "",
  '<script>window.ran = true</script><img src="x" onerror="window.ran = true">',
  "",
].join("\n");

test("a changed Markdown file previews as it reads", async ({ page, hub }) => {
  hub.project("app").submit("docs", "Document setup", { "README.md": doc });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });

  await diff.getByRole("button", { name: "Preview" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toHaveAccessibleName("README.md");
  await expect(
    dialog.getByRole("heading", { level: 1, name: "Setup" }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("heading", { level: 2, name: "Steps" }),
  ).toBeVisible();
  await expect(dialog.locator("strong")).toHaveText("first");
  await expect(dialog.locator("em")).toHaveText("read");
  await expect(dialog.getByText("snake_case_names as written")).toBeVisible();

  // The web opens in a new tab; a path in the repo is only named.
  const guide = dialog.getByRole("link", { name: "guide" });
  await expect(guide).toHaveAttribute("href", "https://example.com/guide");
  await expect(guide).toHaveAttribute("target", "_blank");
  await expect(dialog.getByRole("link", { name: "the notes" })).toHaveCount(0);
  await expect(dialog.getByText("the notes")).toHaveAttribute(
    "title",
    "docs/notes.md",
  );

  const steps = dialog.locator("ol > li");
  await expect(steps).toHaveCount(2);
  await expect(steps.nth(1).locator("ul > li")).toHaveText([
    "set PORT",
    "set HOME",
  ]);
  await expect(dialog.locator("blockquote")).toHaveText(
    "Back up the database first.",
  );
  await expect(dialog.locator("pre")).toHaveText("mg hub start --port 0");
  await expect(dialog.getByRole("table")).toContainText("machine output");

  // Nothing in the file loads or runs.
  await expect(dialog.getByText("[image: the inbox]")).toBeVisible();
  await expect(dialog.locator("img")).toHaveCount(0);
  await expect(dialog.locator("script")).toHaveCount(0);
  await expect(
    dialog.getByText("<script>window.ran = true</script>"),
  ).toBeVisible();
  expect(await page.evaluate(() => "ran" in window)).toBe(false);

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});

test("a deleted Markdown file has no preview", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ "OLD.md": "# Old\n" });
  app.submit("docs", "Drop old docs", { "OLD.md": null, "a.txt": "a\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("a", { exact: true })).toBeVisible();
  await expect(diff.getByRole("button", { name: "Preview" })).toHaveCount(0);
});
