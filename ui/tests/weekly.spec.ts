import { join } from "node:path";
import { expect, test } from "./hub";

test("weekly closures show an empty Monday–Sunday calendar", async ({
  page,
  hub,
}) => {
  await page.goto(hub.url);
  const stats = page.getByRole("region", { name: "Closed this week" });
  await expect(stats).toContainText("No items closed this week yet.");
  await expect(stats.getByRole("columnheader")).toHaveText([
    "Repository",
    /Mon\d+/,
    /Tue\d+/,
    /Wed\d+/,
    /Thu\d+/,
    /Fri\d+/,
    /Sat\d+/,
    /Sun\d+/,
    "Total",
  ]);
  await expect(stats.locator("th[aria-current=date]")).toHaveCount(1);
  await expect(stats.locator("tfoot td").last()).toHaveText("0");
});

test("weekly closures count merges and abandons per repository and update live", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  const web = hub.project("web");
  app.submit("parser", "Add the parser", { "parser.rs": "fn parse() {}\n" });
  app.submit("lexer", "Replace the lexer", { "lexer.rs": "fn lex() {}\n" });
  web.submit("search", "Add search", {
    "search.ts": "export const search = true;\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await page.getByRole("button", { name: "Merge", exact: true }).click();
  await expect(
    page.getByRole("region", { name: "Merged", exact: true }),
  ).toBeVisible();
  await page.getByRole("link", { name: "mergegate", exact: true }).click();
  const stats = page.getByRole("region", { name: "Closed this week" });
  await expect(stats.getByRole("heading")).toHaveText("Closed this week1");
  hub.mg(["abandon", "app#2"]);
  hub.mg(["abandon", "web#1"]);
  await expect(stats.getByRole("heading")).toHaveText("Closed this week3");
  const appRow = stats
    .getByRole("row")
    .filter({ has: page.getByRole("rowheader", { name: "app", exact: true }) });
  const webRow = stats
    .getByRole("row")
    .filter({ has: page.getByRole("rowheader", { name: "web", exact: true }) });
  await expect(appRow.getByRole("cell").last()).toHaveText("2");
  await expect(webRow.getByRole("cell").last()).toHaveText("1");
  await expect(stats.locator("tfoot td").last()).toHaveText("3");
  const todayIndex = await stats
    .locator("th[aria-current=date]")
    .evaluate((th) => (th as HTMLTableCellElement).cellIndex - 1);
  await expect(appRow.getByRole("cell").nth(todayIndex)).toHaveText("2");
  await expect(webRow.getByRole("cell").nth(todayIndex)).toHaveText("1");
  await expect(page.getByText("Closed today (3)")).toBeVisible();

  const out = process.env.MG_WEEKLY_DEMO_DIR;
  if (out) {
    await page.screenshot({
      path: join(out, "weekly-closures.png"),
      fullPage: true,
    });
    await page.emulateMedia({ colorScheme: "dark" });
    await page.screenshot({
      path: join(out, "weekly-closures-dark.png"),
      fullPage: true,
    });
  }
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(stats.getByRole("table")).toBeVisible();
  expect(await stats.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(
    true,
  );
  if (out)
    await page.screenshot({
      path: join(out, "weekly-closures-mobile.png"),
      fullPage: true,
    });
});
