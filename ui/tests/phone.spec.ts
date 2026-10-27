// Open on phone: while the hub listens on the tailnet, the bar offers the
// page's tailnet link as a QR code a phone on the tailnet scans.

import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Settings } from "../api/Settings";
import { expect, Hub, test } from "./hub";

/**
 * Runs `body` with a hub listening on "the tailnet" at a loopback port,
 * through a stand-in for the `tailscale` CLI, as testkit's `FakeTailscale`.
 */
async function onTailnet(body: (hub: Hub, url: string) => Promise<void>) {
  const dir = mkdtempSync(join(tmpdir(), "mg-tailscale-"));
  const bin = join(dir, "bin");
  mkdirSync(bin);
  const node = {
    ID: "nhub",
    HostName: "hub",
    DNSName: "hub.tail0.ts.net.",
    UserID: 1,
    TailscaleIPs: ["100.64.0.1"],
  };
  writeFileSync(
    join(dir, "status.json"),
    JSON.stringify({
      BackendState: "Running",
      Self: node,
      Peer: {},
      User: { 1: { ID: 1, LoginName: "owner@example.com" } },
    }),
  );
  const tailscale = join(bin, "tailscale");
  writeFileSync(
    tailscale,
    `#!/bin/sh\n[ "$1" = status ] && exec cat '${dir}/status.json'\necho "peer not found" >&2\nexit 1\n`,
  );
  chmodSync(tailscale, 0o755);
  const hub = new Hub({
    PATH: `${bin}:${process.env.PATH ?? ""}`,
    MG_TAILNET_ADDR: "127.0.0.1:0",
    MG_TAILNET_RETRY_MS: "100",
  });
  try {
    const settings = hub.mg(["hub", "set", "listen", "tailnet"]) as Settings;
    const url = settings.tailnet?.url;
    if (!url) throw new Error(`not listening: ${JSON.stringify(settings)}`);
    await body(hub, url);
  } finally {
    hub.stop();
    rmSync(dir, { recursive: true, force: true });
  }
}

test("f2 a hub only on this machine offers no phone link", async ({
  page,
  hub,
}) => {
  await page.goto(hub.url);
  await expect(page.getByRole("banner")).toContainText("shortcuts");
  await expect(page.getByRole("button", { name: "Open on phone" })).toHaveCount(
    0,
  );
  const qr = await page.request.get(`${hub.url}api/hub/qr`);
  expect(qr.status()).toBe(404);
});

test("f2 the inbox shows its tailnet link as a QR code", async ({ page }) => {
  await onTailnet(async (hub, url) => {
    await page.goto(hub.url);
    await page.getByRole("button", { name: "Open on phone" }).click();
    const dialog = page.getByRole("dialog", { name: "Open on your phone" });
    await expect(dialog.getByRole("link")).toHaveText(`${url}/#/`);
    const code = dialog.getByRole("img", { name: `QR code of ${url}/#/` });
    await expect(code).toBeVisible();
    expect(
      await code.evaluate((img: HTMLImageElement) => img.naturalWidth),
    ).toBeGreaterThan(0);

    await page.keyboard.press("Escape");
    await expect(dialog).toHaveCount(0);
  });
});

test("f2 an item's QR code opens that item on the phone", async ({ page }) => {
  await onTailnet(async (hub, url) => {
    hub.project("app").submit("parser", "Add the parser", {
      "src/parser.rs": "fn parse() {}\n",
    });
    await page.goto(`${hub.url}#/items/app%231`);
    await expect(page.getByRole("banner")).toContainText("app#1");
    // The screen adds its own state, such as the file on screen, to the route.
    const route = new URL(page.url()).hash;
    expect(route).toMatch(/^#\/items\/app%231/);

    await page.getByRole("button", { name: "Open on phone" }).click();
    const dialog = page.getByRole("dialog", { name: "Open on your phone" });
    await expect(dialog.getByRole("link")).toHaveText(`${url}/${route}`);
    const qr = await page.request.get(
      `${hub.url}api/hub/qr?route=${encodeURIComponent(route)}`,
    );
    expect(qr.status()).toBe(200);
    expect(qr.headers()["content-type"]).toBe("image/svg+xml");
  });
});
