// Highlighting: a file shows plain, naming the pack that would highlight it,
// until the operator installs that pack; then its hunks and the gaps it
// expands are highlighted, without reloading the page.

import { resolve } from "node:path";
import type { Page, Route } from "@playwright/test";
import type { ItemDiff } from "../api/ItemDiff";
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

/**
 * Holds the item's first `hold` diff requests until the test answers them;
 * the hub answers the rest as they come.
 */
async function holdDiffs(page: Page, hold: number) {
  const held: Route[] = [];
  let passed = 0;
  await page.route(
    (url) => url.pathname === "/api/items/app%231",
    async (route) => {
      if (held.length < hold) {
        held.push(route);
        return;
      }
      await route.fulfill({ response: await route.fetch() });
      passed++;
    },
  );
  return {
    count: () => held.length,
    passed: () => passed,
    /** Answers the `index`th held request with what the hub says now. */
    async answer(index: number, edit = (body: ItemDiff) => body) {
      const route = held[index];
      if (!route) throw new Error(`no request ${index}`);
      const response = await route.fetch();
      const body = edit((await response.json()) as ItemDiff);
      await route.fulfill({ response, json: body });
    },
  };
}

/** `body` as the hub had it before the pass highlighted it. */
const withoutSpans = (body: ItemDiff): ItemDiff => ({
  ...body,
  files: body.files.map((file) =>
    file.content.kind === "text"
      ? {
          ...file,
          content: {
            ...file.content,
            hunks: file.content.hunks.map((hunk) => ({ ...hunk, spans: [] })),
          },
        }
      : file,
  ),
});

test("spans that land while the diff loads still show", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "data.json": json([]) });
  app.submit("work", "Tune", { "data.json": json([10, 50]) });
  const diffs = await holdDiffs(page, 1);
  await page.goto(`${hub.url}#/items/app%231`);
  // The event stream's opening refetches, and the hub answers before it
  // has a pack.
  await expect.poll(diffs.count).toBe(1);
  await expect.poll(diffs.passed).toBeGreaterThan(0);

  // The hub highlights, and says so, before the page has its first answer.
  hub.mg(["lang", "add", "--from", jsonPack]);
  await expect
    .poll(async () => {
      const response = await fetch(`${hub.url}api/items/app%231`);
      const body = (await response.json()) as ItemDiff;
      const file = body.files[0];
      return file?.content.kind === "text" && file.content.hunks[0]?.spans[0];
    })
    .toBeTruthy();
  await page.waitForTimeout(500);
  await diffs.answer(0, withoutSpans);
  await page.unrouteAll();

  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText('"k10": 1000,', { exact: true })).toBeVisible();
  await expect(diff.locator("[data-tok]").first()).toBeVisible();
});

test("spans answered after an older refetch still show", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "data.json": json([]) });
  app.submit("work", "Tune", { "data.json": json([10, 50]) });
  await page.goto(`${hub.url}#/items/app%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("No json language pack")).toBeVisible();

  // Installing a pack says so twice: once installed, once highlighted. The
  // first refetch's answer, from before the pass, arrives first.
  const diffs = await holdDiffs(page, 2);
  hub.mg(["lang", "add", "--from", jsonPack]);
  await expect.poll(diffs.count).toBe(2);
  await diffs.answer(0, withoutSpans);
  await expect(diff.getByText("No json language pack")).toHaveCount(0);
  await diffs.answer(1);

  await expect(diff.locator("[data-tok]").first()).toBeVisible();
});
