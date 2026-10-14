// Highlighting: a file shows plain, naming the pack that would highlight it,
// until the operator installs that pack; then its hunks and the gaps it
// expands are highlighted, without reloading the page.

import { resolve } from "node:path";
import { expect, test } from "./hub";

/** A pack built by `langs/build.mjs`, kept small enough to commit. */
const jsonPack = resolve(
  import.meta.dirname,
  "../../crates/highlight/tests/fixtures/json-0.24.8.tar.gz",
);

/** 61 lines of JSON; the keys on `changed` lines get new values. */
const json = (changed: number[]) => {
  const entries = Array.from({ length: 58 }, (_, i) => {
    const n = i + 2;
    return `  "k${n}": ${changed.includes(n) ? n * 100 : n},`;
  });
  return `{\n${entries.join("\n")}\n  "end": 0\n}\n`;
};

test("installing a pack highlights open diffs", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ "data.json": json([]) });
  app.submit("work", "Tune", { "data.json": json([10, 50]) });
  await page.goto(`${hub.url}#/items/app%231`);

  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText('"k10": 1000,', { exact: true })).toBeVisible();
  await expect(diff.getByText("No json language pack")).toBeVisible();
  await expect(diff.locator("[data-tok]")).toHaveCount(0);

  hub.mg(["lang", "add", "--from", jsonPack]);
  const key = diff.locator(
    '[data-line-type="change-addition"] [data-tok="string"]',
    { hasText: '"k10"' },
  );
  await expect(key).toBeVisible();
  // Pierre's light string colour, as Shiki would show it.
  await expect(key).toHaveCSS("color", "rgb(25, 159, 67)");
  await expect(
    diff.locator('[data-tok="number"]', { hasText: /^1000$/ }),
  ).toBeVisible();
  await expect(diff.getByText("No json language pack")).toHaveCount(0);

  // The lines between the hunks come from the blobs, with their spans.
  const gap = diff.locator('[data-tok="string"]', { hasText: '"k30"' });
  await expect(gap).toHaveCount(0);
  await page
    .locator("[data-separator]", { hasText: "33 unmodified lines" })
    .getByRole("button")
    .first()
    .click();
  await expect(gap).toBeVisible();
});
