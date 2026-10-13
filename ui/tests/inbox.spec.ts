// The inbox part of F6: open the hub, see open items grouped, move with
// `j/k`, open with `enter`. State comes from real `mg` commands.

import { expect, test } from "./hub";

test("f6 an empty inbox says how items arrive", async ({ page, hub }) => {
  await page.goto(hub.url);
  const needsYou = page.getByRole("region", { name: "Needs you" });
  await expect(needsYou).toContainText("Nothing to review");
  await expect(needsYou).toContainText("mg submit");
});

test("f6 lists open items with their figures", async ({ page, hub }) => {
  const app = hub.project("app");
  app.submit("parser", "Add the parser", {
    "src/parser.rs": "fn parse() {}\n",
    "src/lexer.rs": "fn lex() {}\nfn peek() {}\n",
  });
  await page.goto(hub.url);

  const needsYou = page.getByRole("region", { name: "Needs you" });
  await expect(needsYou.getByRole("heading")).toHaveText(/^Needs you\s*1$/);
  const row = needsYou.getByRole("listitem");
  await expect(row).toHaveCount(1);
  for (const text of ["app#1", "Add the parser", "+3", "2 files"]) {
    await expect(row).toContainText(text);
  }
  await expect(row).toContainText("now");
  // Nothing was removed, so the deletions cell stays empty.
  await expect(row).not.toContainText("−");
});

test("f6 a submit shows without a reload", async ({ page, hub }) => {
  const app = hub.project("app");
  await page.goto(hub.url);
  const rows = page.getByRole("listitem");
  await expect(page.getByText("Nothing to review")).toBeVisible();

  app.submit("parser", "Add the parser", { "parser.rs": "fn parse() {}\n" });
  await expect(rows).toHaveCount(1);
  await expect(rows).toContainText("1 file");

  app.revise("Add the lexer", { "lexer.rs": "fn lex() {}\n" });
  await expect(rows).toContainText("2 files");
});

test("f6 j and k move the selection, enter opens the item", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("first", "First change", { "a.txt": "a\n" });
  app.submit("second", "Second change", { "b.txt": "b\n" });
  await page.goto(hub.url);

  const selected = page.locator("li[aria-current=true]");
  await expect(selected).toContainText("app#1");
  await page.keyboard.press("j");
  await expect(selected).toContainText("app#2");
  await page.keyboard.press("j");
  await expect(selected).toContainText("app#2");
  await page.keyboard.press("k");
  await expect(selected).toContainText("app#1");
  await page.keyboard.press("j");

  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/#\/items\/app%232$/);
  await expect(page.getByRole("region", { name: "app#2" })).toBeVisible();

  // Back on the inbox, the selection is where it was.
  await page.keyboard.press("Escape");
  await expect(selected).toContainText("app#2");
});

test("f6 clicking a row opens its item", async ({ page, hub }) => {
  hub.project("app").submit("parser", "Add the parser", { "p.rs": "p\n" });
  await page.goto(hub.url);
  await page.getByText("Add the parser").click();
  await expect(page).toHaveURL(/#\/items\/app%231$/);
});

// tech-stack.md §9: first paint under 100 ms. Measured everywhere, enforced
// with MG_ENFORCE_BUDGETS=1, which CI sets on Ubuntu only.
test("the inbox paints within budget", async ({ page, hub }, info) => {
  const app = hub.project("app");
  for (const n of [1, 2, 3, 4, 5]) {
    app.submit(`change-${n}`, `Change ${n}`, { [`${n}.txt`]: `${n}\n` });
  }
  await page.goto(hub.url);
  await expect(page.getByRole("listitem")).toHaveCount(5);

  const timings: number[] = [];
  for (let run = 0; run < 5; run++) {
    await page.reload({ waitUntil: "commit" });
    // Checked every frame, so the time is that of the frame with the rows.
    const painted = await page.waitForFunction(
      () => document.querySelectorAll("li").length === 5 && performance.now(),
      undefined,
      { polling: "raf" },
    );
    timings.push((await painted.jsonValue()) as number);
  }
  const median = timings.sort((a, b) => a - b)[2] ?? Infinity;
  info.annotations.push({
    type: "first paint",
    description: `${median.toFixed(1)} ms (median of ${timings.length})`,
  });
  if (process.env.MG_ENFORCE_BUDGETS === "1") expect(median).toBeLessThan(100);
});
