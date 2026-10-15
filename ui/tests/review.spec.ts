// The viewing part of F6: open an item, see every changed file in one
// column, walk it with `n/p`, `j/k` and `]/[`, expand the gaps between hunks.

import { expect, type Hub, test } from "./hub";

const lines = (count: number, line = (n: number) => `line ${n}`) =>
  Array.from({ length: count }, (_, i) => `${line(i + 1)}\n`).join("");

/**
 * app#1: `a.txt` changes lines 10 and 50, leaving 33 lines between its two
 * hunks; `gone.txt` is deleted, `logo.png` is binary, `new.txt` is new.
 * The heuristics rank the text files low as docs, so `logo.png` comes first.
 */
function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "a.txt": lines(60), "gone.txt": "bye\n" });
  app.submit("work", "Change things", {
    "a.txt": lines(60, (n) =>
      n === 10 || n === 50 ? `changed ${n}` : `line ${n}`,
    ),
    "gone.txt": null,
    "logo.png": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 1]),
    "new.txt": "hello\n",
  });
}

test("f6 an item shows every changed file", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByRole("heading")).toHaveText("Change things");
  const files = screen.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^logo\.png$/,
    /^a\.txt\s*ranked low: docs\s*\+2\s*−2$/,
    /^gone\.txt\s*ranked low: docs\s*−1$/,
    /^new\.txt\s*ranked low: docs\s*\+1$/,
  ]);

  const diff = screen.getByRole("region", { name: "Diff" });
  for (const text of ["changed 10", "changed 50", "bye", "hello"]) {
    await expect(diff.getByText(text, { exact: true })).toBeVisible();
  }
  await expect(diff.getByText("Binary file not shown")).toBeVisible();
  // Three lines of context, the rest folded.
  await expect(diff.getByText("line 7", { exact: true })).toBeVisible();
  await expect(diff.getByText("line 6", { exact: true })).toHaveCount(0);
});

test("f6 n and p move between files", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const current = page
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  // The diff reports the scroll a key caused a frame later; the next key
  // waits for it, as the operator would.
  const press = async (key: string, file: string) => {
    await page.keyboard.press(key);
    await expect(current).toContainText(file);
    await page.evaluate(
      () =>
        new Promise((done) =>
          requestAnimationFrame(() => requestAnimationFrame(done)),
        ),
    );
  };
  await expect(current).toContainText("logo.png");
  await press("n", "a.txt");
  await press("n", "gone.txt");
  await press("n", "new.txt");
  // The last file stays current.
  await press("n", "new.txt");
  await press("p", "gone.txt");
});

test("f6 j, k, ] and [ move the line cursor", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  // The line's content cell; its gutter carries the mark too.
  const cursor = page.locator("[data-line][data-selected-line]");
  await expect(page.getByText("changed 10", { exact: true })).toBeVisible();
  await expect(cursor).toHaveCount(0);
  await page.keyboard.press("j");
  await expect(cursor).toHaveText("line 7");
  await page.keyboard.press("j");
  await expect(cursor).toHaveText("line 8");
  await page.keyboard.press("k");
  await expect(cursor).toHaveText("line 7");

  await page.keyboard.press("]");
  await expect(cursor).toHaveText("line 47");
  // The next hunk is the next file's.
  await page.keyboard.press("]");
  await expect(cursor).toHaveText("bye");
  // The diff reports the scroll that showed the line a frame later; the file
  // moved to stays current past it.
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
  await expect(
    page.getByRole("navigation", { name: "Files" }).locator("[aria-current]"),
  ).toContainText("gone.txt");
  await page.keyboard.press("[");
  await page.keyboard.press("[");
  await expect(cursor).toHaveText("line 7");

  // `n` puts the cursor on the file's first line.
  await page.keyboard.press("n");
  await expect(cursor).toHaveText("bye");
});

test("f6 a gap between hunks expands", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const line = page.getByText("line 30", { exact: true });
  await expect(line).toHaveCount(0);
  await page
    .locator("[data-separator]", { hasText: "33 unmodified lines" })
    .getByRole("button")
    .first()
    .click();
  await expect(line).toBeVisible();
});

test("f6 v marks the current file viewed and moves on", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  const current = screen
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  const changed = page.getByText("changed 10", { exact: true });
  await expect(changed).toBeVisible();
  await page.keyboard.press("n");
  await expect(current).toContainText("a.txt");
  await page.keyboard.press("v");
  await expect(current).toContainText("gone.txt");
  await expect(screen.getByLabel("Viewed a.txt")).toBeChecked();
  await expect(screen.getByText("1/4 viewed")).toBeVisible();
  // Collapsed to its header, and out of the cursor's way.
  await expect(changed).toHaveCount(0);
  const cursor = page.locator("[data-line][data-selected-line]");
  await expect(cursor).toHaveText("bye");
  await page.keyboard.press("k");
  await expect(cursor).toHaveText("bye");

  // The hub keeps it.
  await page.reload();
  await expect(screen.getByLabel("Viewed a.txt")).toBeChecked();
  await expect(page.getByText("bye", { exact: true })).toBeVisible();
  await expect(changed).toHaveCount(0);

  await screen.getByRole("listitem").filter({ hasText: "a.txt" }).click();
  await page.keyboard.press("v");
  await expect(screen.getByLabel("Viewed a.txt")).not.toBeChecked();
  await expect(changed).toBeVisible();
  await expect(current).toContainText("a.txt");
  await expect(screen.getByText("0/4 viewed")).toBeVisible();
});

test("f6 a file's checkbox marks it viewed", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  await files.getByLabel("Viewed new.txt").check();
  await expect(page.getByText("1/4 viewed")).toBeVisible();
  await expect(page.getByText("hello", { exact: true })).toHaveCount(0);
  // The checkbox doesn't keep the keys.
  await page.keyboard.press("n");
  await expect(files.locator("li[aria-current=true]")).toContainText("a.txt");

  await page.reload();
  await expect(files.getByLabel("Viewed new.txt")).toBeChecked();
});

test("f6 escape goes back to the inbox", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.getByText("changed 10", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("region", { name: "Needs you" })).toBeVisible();
});

// tech-stack.md §9: a 5k-line diff scrolls at 60 fps. Measured everywhere,
// enforced with MG_ENFORCE_BUDGETS=1, which CI sets on Ubuntu only.
test("a 5k-line diff scrolls within budget", async ({ page, hub }, info) => {
  const files: Record<string, string> = {};
  for (let file = 1; file <= 50; file++) {
    files[`src/module_${file}.rs`] = lines(
      100,
      (n) => `    let value_${n} = compute(${file}, ${n}).unwrap_or_default();`,
    );
  }
  hub.project("big").submit("big", "A large change", files);
  await page.goto(`${hub.url}#/items/big%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(
    diff.getByText("let value_1 =", { exact: false }).first(),
  ).toBeVisible();

  // A fast fling, ten lines a frame from top to bottom, so CodeView renders
  // fresh rows every frame; the time between frames is what the reviewer
  // would see.
  const frames = await diff.evaluate(
    (view) =>
      new Promise<number[]>((resolve) => {
        const gaps: number[] = [];
        let last = performance.now();
        const step = (now: number) => {
          gaps.push(now - last);
          last = now;
          view.scrollTop += 200;
          const end = view.scrollTop + view.clientHeight >= view.scrollHeight;
          if (end || gaps.length > 2000) resolve(gaps.slice(1));
          else requestAnimationFrame(step);
        };
        requestAnimationFrame(step);
      }),
  );
  const sorted = frames.sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.floor((sorted.length - 1) * q)] ?? 0;
  info.annotations.push({
    type: "scroll frames",
    description: `p50 ${at(0.5).toFixed(1)} ms, p95 ${at(0.95).toFixed(1)} ms, max ${at(1).toFixed(1)} ms over ${sorted.length} frames`,
  });
  expect(sorted.length).toBeGreaterThan(100);
  // A frame dropped at 60 fps takes two refreshes, 33 ms.
  if (process.env.MG_ENFORCE_BUDGETS === "1") expect(at(0.95)).toBeLessThan(25);
});
