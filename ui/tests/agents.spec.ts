// The inbox offers to set up the coding agents on the hub's machine that
// don't load mergegate's skill yet, as `mg skill setup` does.

import { existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "./hub";

test("the inbox offers to set up the agents found", async ({ page, hub }) => {
  mkdirSync(join(hub.user, ".claude"));
  mkdirSync(join(hub.user, ".codex"));
  await page.goto(hub.url);

  const prompt = page.getByRole("region", { name: "Agent setup" });
  await expect(prompt).toContainText(
    "Claude Code and Codex don't load mergegate's skill yet",
  );
  await prompt.getByRole("button", { name: "Set up agents" }).click();

  await expect(prompt).toBeHidden();
  // Codex runs the new hook only once the operator trusts it.
  await expect(
    page.getByRole("list", { name: "Agent setup notes" }),
  ).toContainText("To finish, trust the new hook with /hooks in Codex.");
  expect(existsSync(join(hub.user, ".claude/skills/mergegate/SKILL.md"))).toBe(
    true,
  );
  expect(existsSync(join(hub.user, ".agents/skills/mergegate/SKILL.md"))).toBe(
    true,
  );
});

test("an agent already set up isn't offered", async ({ page, hub }) => {
  mkdirSync(join(hub.user, ".claude"));
  hub.mg(["skill", "setup"]);
  await page.goto(hub.url);
  await expect(page.getByRole("region", { name: "Needs you" })).toBeVisible();
  await expect(page.getByRole("region", { name: "Agent setup" })).toBeHidden();
});
