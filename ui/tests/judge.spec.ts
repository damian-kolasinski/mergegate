// The judge part of F12: a model the operator picked scores each file, its
// high scores raise files ahead of the guide's order, and a trusted file it
// is sure needs a look comes back into review.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, Hub, test } from "./hub";
import { FakeJev } from "./jev";

const GUIDE = `Keeps sessions alive across restarts.

## Files

- \`README.md\` — the new setting
- \`src/main.rs\` — wires the store in
`;

test("f12 the judge raises and pulls back files", async ({ page }) => {
  const jev = await FakeJev.start({
    // Jev scores 0 to 9; the hub shows them as 1 to 100.
    scores: { "src/session.rs": 9, "README.md": 2, "src/main.rs": 4 },
    looks: { "Cargo.lock": 0.9 },
  });
  const hub = new Hub(jev.env);
  try {
    const app = hub.project("app");
    await hub.rules(
      "app",
      `[[rule]]\nid = "R1"\ntier = "trusted"\npath = "**/*.lock"\nreason = "regenerated"\n`,
    );
    const guide = join(hub.root, "guide.md");
    writeFileSync(guide, GUIDE);
    app.submit("sessions", "Keep sessions", {
      "README.md": "# app\n\nsessions = disk\n",
      "src/main.rs": "fn main() { session::load() }\n",
      "src/session.rs": "pub fn load() {}\n",
      "Cargo.lock": "version = 4\n",
    });
    hub.mg(["submit", "--guide", guide], app.dir);
    await page.goto(`${hub.url}#/items/app%231`);

    const files = page
      .getByRole("region", { name: "app#1" })
      .getByRole("navigation", { name: "Files" });
    await expect(files.getByRole("listitem").first()).toContainText(
      "README.md",
    );
    // Picked with the item open: its scores land without a reload.
    hub.mg(["judge", "set", "jev"], app.dir);
    await expect(files.getByRole("listitem")).toHaveText([
      /^src\/session\.rs\s*raised by the judge 100\/100\s*\+1$/,
      /^README\.md\s*the new setting\s*judged 23\/100\s*\+3$/,
      /^src\/main\.rs\s*wires the store in\s*judged 45\/100\s*\+1$/,
      /^Cargo\.lock\s*judged 45\/100\s*promoted by the judge; trusted by R1: `\*\*\/\*\.lock` — regenerated\s*\+1$/,
    ]);
    // Nothing is left trusted, so there is no group to open.
    await expect(files.getByRole("button", { name: /^Trusted/ })).toHaveCount(
      0,
    );
  } finally {
    hub.stop();
    jev.stop();
  }
});

test("f12 without a judge the heuristics explain the order", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("sessions", "Keep sessions", {
    "README.md": "# app\n",
    "src/session.rs": "pub fn load() {}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^src\/session\.rs\s*ranked high: changes a declaration\s*\+1$/,
    /^README\.md\s*ranked low: docs\s*\+1$/,
  ]);
});
