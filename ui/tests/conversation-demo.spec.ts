// Screenshots of the Conversation list: what waits on the operator first,
// the rest folded under it and then opened, with a bot's comment a later
// revision outdated lapsed, in light and dark. Skipped unless MG_SHOTS_DIR
// names where to write them:
//
//   MG_SHOTS_DIR=$(mktemp -d) pnpm --dir ui e2e conversation-demo --project chromium

import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "./hub";

const out = process.env.MG_SHOTS_DIR ?? "";

test.skip(!out, "MG_SHOTS_DIR is unset");
// Short, so the comment voted up goes to the agent while the test waits.
test.use({
  viewport: { width: 1280, height: 980 },
  deviceScaleFactor: 2,
  quietMs: "1000",
});

const login = (check: boolean) =>
  [
    "pub fn login(name: &str, password: &str) -> Result<Session> {",
    ...(check ? ["    let user = users::find(name).unwrap();"] : []),
    "    verify(password)?;",
    "    let session = Session::start(name);",
    "    audit::log(name);",
    "    Ok(session)",
    "}",
    "",
  ].join("\n");

async function shoot(page: Page, name: string) {
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.mouse.move(10, 900);
    await page.screenshot({
      path: join(out, `${name}-${scheme}.png`),
      animations: "disabled",
    });
  }
}

test("conversation list screenshots", async ({ page, github }) => {
  test.setTimeout(120_000);
  const { hub } = github;
  const app = hub.githubProject("app", github.github);
  app.submit("login", "Add login", {
    "src/login.rs": login(true),
    "src/session.rs":
      "pub struct Session {\n    pub name: String,\n    pub started: u64,\n}\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const conversation = screen.getByRole("button", { name: /^Conversation/ });
  const list = page.getByRole("complementary", { name: "Conversation" });

  // A person's comment goes to the agent once the reviewers go quiet, and
  // the agent asks about it.
  await github.github.comment(1, {
    login: "alice",
    body: "Should login and session handling live apart?",
  });
  await expect(conversation).toContainText("1 to vote on");
  await conversation.click();
  await list
    .getByRole("article", { name: "Comment c_1" })
    .getByRole("button", { name: "Send to agent" })
    .click();
  await page.keyboard.press("Escape");
  await expect
    .poll(() => (hub.mg(["feedback", "app#1"]) as { events: [] }).events)
    .toHaveLength(1);
  hub.mg([
    "reply",
    "c_1",
    "--status",
    "question",
    "--body",
    "Into a `session` module of its own, or only a file apart?",
  ]);

  // Then the bots review.
  await github.github.comment(1, {
    login: "codex[bot]",
    body: "**P1** `unwrap` panics on an unknown user; return an error instead.",
    path: "src/login.rs",
    line: 2,
  });
  await github.github.comment(1, {
    login: "codex[bot]",
    body: "**P2** Audit failed logins too, not only successful ones.",
    path: "src/login.rs",
    line: 5,
  });
  await github.github.comment(1, {
    login: "coderabbitai[bot]",
    body: "`started` reads better as a `SystemTime` than a bare `u64`.",
    path: "src/session.rs",
    line: 3,
  });
  await github.github.comment(1, {
    login: "coderabbitai[bot]",
    body: "<!-- This is an auto-generated comment: summarize by coderabbit.ai -->\n## Walkthrough\n\nAdds login and sessions.",
  });
  await expect(conversation).toContainText("3 to vote on");

  // The agent's next revision drops the line the first finding was on.
  app.revise("Look the user up once", { "src/login.rs": login(false) });
  await page.reload();
  // The poller lapses it once it reads the comments again.
  await expect(conversation).toContainText("2 to vote on");

  await conversation.click();
  await expect(
    list
      .getByRole("region", { name: "Needs you" })
      .getByRole("listitem", { name: /^c_\d+$/ }),
  ).toHaveCount(3);
  await shoot(page, "conversation-needs-you");

  await list.getByRole("button", { name: /Everything else/ }).click();
  await list.getByRole("button", { name: "Open comment c_2" }).click();
  await expect(
    list.getByRole("article", { name: "Comment c_2" }),
  ).toContainText("Lapsed");
  await shoot(page, "conversation-everything-else");
});
