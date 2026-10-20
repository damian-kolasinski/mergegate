// A page left open across a hub upgrade reloads onto the new hub's UI,
// unless that would lose what the operator hasn't saved.

import type { Page } from "@playwright/test";
import type { Health } from "../api/Health";
import { expect, type Hub, test } from "./hub";

type Faked = Window & {
  /** Hides or shows the page, which closes and reopens its event stream. */
  setHidden: (hidden: boolean) => void;
  /** Set on a page to tell it from the one a reload brings. */
  stale?: true;
};

/**
 * Lets `upgrade` stand in for restarting the hub onto a new `mg`: what
 * `/api/health` answers, and the reconnect a restart causes.
 */
async function fakeUpgrades(page: Page) {
  await page.addInitScript(() => {
    const fetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
      const response = await fetch(input, init);
      const version = sessionStorage.getItem("test:version");
      if (!version || !String(input).endsWith("/api/health")) return response;
      const health = (await response.json()) as Health;
      return Response.json({ ...health, version });
    };
    let hidden = false;
    Object.defineProperty(document, "hidden", { get: () => hidden });
    (window as unknown as Faked).setHidden = (next) => {
      hidden = next;
      document.dispatchEvent(new Event("visibilitychange"));
    };
  });
}

async function upgrade(page: Page, version: string) {
  await page.evaluate((version) => {
    const faked = window as unknown as Faked;
    faked.stale = true;
    sessionStorage.setItem("test:version", version);
    faked.setHidden(true);
    faked.setHidden(false);
  }, version);
}

const stale = (page: Page) =>
  page.evaluate(() => (window as unknown as Faked).stale ?? false);

/** Opens `path` once the header shows the hub's real version. */
async function open(page: Page, hub: Hub, path = "") {
  await fakeUpgrades(page);
  const health = (await (
    await page.request.get(`${hub.url}api/health`)
  ).json()) as Health;
  await page.goto(`${hub.url}${path}`);
  await expect(
    page.getByRole("banner").getByText(health.version, { exact: true }),
  ).toBeVisible();
}

/** Opens a submitted change's item. */
async function openItem(page: Page, hub: Hub) {
  const app = hub.project("app");
  app.seed({ "login.rs": "fn login() {}\n" });
  app.submit("work", "Add login", {
    "login.rs": "fn login(password: &str) {}\n",
  });
  await open(page, hub, "#/items/app%231");
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("fn login(password: &str) {}")).toBeVisible();
}

test("f6 a page open across an upgrade reloads onto the new UI", async ({
  page,
  hub,
}) => {
  await open(page, hub);

  await upgrade(page, "9.9.9");
  const banner = page.getByRole("banner");
  await expect(banner.getByText("9.9.9", { exact: true })).toBeVisible();
  expect(await stale(page)).toBe(false);
});

test("f6 a written summary doesn't hold the reload, and outlives it", async ({
  page,
  hub,
}) => {
  await openItem(page, hub);
  const summary = page.getByRole("textbox", { name: "Review summary" });
  // Typed rather than filled: the browser asks before an unload only once
  // the operator has interacted with the page.
  await summary.click();
  await page.keyboard.type("Looks close.");
  const asked: string[] = [];
  page.on("dialog", (dialog) => {
    asked.push(dialog.type());
    void dialog.accept();
  });

  await upgrade(page, "9.9.9");
  const banner = page.getByRole("banner");
  await expect(banner.getByText("9.9.9", { exact: true })).toBeVisible();
  expect(await stale(page)).toBe(false);
  expect(asked).toEqual([]);
  await expect(summary).toHaveValue("Looks close.");
});

test("f6 an open comment holds the reload until it's saved", async ({
  page,
  hub,
}) => {
  await openItem(page, hub);
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  const editor = page.getByRole("textbox", {
    name: "Comment on login.rs line 1",
  });
  await expect(editor).toBeFocused();
  await page.keyboard.type("Check the password.");

  await upgrade(page, "9.9.9");
  const banner = page.getByRole("banner");
  await expect(banner.getByText("9.9.9 · reload pending")).toBeVisible();
  expect(await stale(page)).toBe(true);
  await expect(editor).toHaveValue("Check the password.");

  await page.keyboard.press("ControlOrMeta+Enter");
  await expect(banner.getByText("9.9.9", { exact: true })).toBeVisible();
  expect(await stale(page)).toBe(false);
  await expect(
    page.getByRole("article", { name: "Comment c_1" }),
  ).toContainText("Check the password.");
});
