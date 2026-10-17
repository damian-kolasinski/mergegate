// External review from the UI: comments on an item's pull request show on the review
// screen as they arrive, held until the reviewers go quiet or the operator
// syncs; the operator agrees with or overrides the agent's rejection of one.

import { expect, test } from "./hub";

test("f5 shows comments from GitHub and syncs them to the agent", async ({
  page,
  github,
}) => {
  const app = github.hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await page.goto(`${github.hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });

  await github.github.comment(1, {
    login: "codex[bot]",
    body: "Hash the password.",
    path: "src/login.rs",
    line: 1,
  });
  await github.github.comment(1, { login: "alice", body: "Needs a test." });

  // The inline one sits under its line, held for the agent.
  const inline = screen.getByRole("article", { name: "Comment c_1" });
  await expect(inline).toContainText("Hash the password.");
  await expect(inline.getByText("codex[bot]")).toBeVisible();
  await expect(inline.getByText("Bot", { exact: true })).toBeVisible();
  await expect(inline.getByText("Not sent yet", { exact: true })).toBeVisible();
  await expect(inline.getByRole("link", { name: "GitHub" })).toHaveAttribute(
    "href",
    /\/acme\/app\/pull\/1#/,
  );

  // The other is on no line, so only the Conversation list has it.
  await screen.getByRole("button", { name: "Conversation 2" }).click();
  const entry = page.getByRole("listitem", { name: "c_2" });
  await expect(entry.getByText("Pull request @r1")).toBeVisible();
  await expect(entry).toContainText("Needs a test.");
  await expect(entry.getByText("alice")).toBeVisible();

  await screen.getByRole("button", { name: "More actions" }).click();
  await screen.getByRole("button", { name: "Fetch GitHub comments" }).click();
  await expect(
    screen.getByText("Up to date, 2 sent to the agent"),
  ).toBeVisible();
  await expect(
    inline.getByText("Sent to agent", { exact: true }),
  ).toBeVisible();
  await expect(entry.getByText("Not sent yet", { exact: true })).toHaveCount(0);
});

test("the operator agrees with a rejection or overrides it", async ({
  page,
  github,
}) => {
  const { hub } = github;
  const app = hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await github.github.comment(1, { login: "alice", body: "Rename it." });
  await github.github.comment(1, { login: "alice", body: "Split it." });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await screen.getByRole("button", { name: "More actions" }).click();
  await screen.getByRole("button", { name: "Fetch GitHub comments" }).click();
  await expect(screen.getByText(/sent to the agent$/)).toBeVisible();
  await page.keyboard.press("Escape");
  hub.mg(["feedback", "app#1"]);
  for (const comment of ["c_1", "c_2"]) {
    hub.mg(["reply", comment, "--status", "rejected", "--body", "No."]);
  }

  await screen.getByRole("button", { name: /^Conversation/ }).click();
  const rename = page.getByRole("article", { name: "Comment c_1" });
  const split = page.getByRole("article", { name: "Comment c_2" });
  await expect(rename.getByText("Rejected", { exact: true })).toBeVisible();

  await split.getByRole("button", { name: "Accept rejection" }).click();
  await expect(
    split.getByText("Rejection accepted", { exact: true }),
  ).toBeVisible();
  await expect(split.getByRole("button", { name: "Override" })).toHaveCount(0);

  await rename.getByRole("button", { name: "Override — must fix" }).click();
  await page
    .getByRole("textbox", { name: "Override the rejection of c_1" })
    .fill("The old name misleads.");
  await rename.getByRole("button", { name: "Send override" }).click();
  await expect(rename.getByText("Override", { exact: true })).toBeVisible();
  await expect(rename).toContainText("The old name misleads.");
  await expect(
    rename.getByRole("button", { name: "Accept rejection" }),
  ).toHaveCount(0);

  // The agent has it at once, with no review.
  const feedback = hub.mg(["feedback", "app#1"]) as {
    events: { type: string; reply: { body: string } }[];
  };
  expect(feedback.events.map((e) => e.type)).toEqual(["comment.override"]);
  expect(feedback.events[0]?.reply.body).toBe("The old name misleads.");
});

test("a comment from GitHub renders its Markdown, and nothing that runs", async ({
  page,
  github,
}) => {
  const app = github.hub.githubProject("app", github.github);
  app.submit("login", "Add login", { "src/login.rs": "fn login() {}\n" });
  await github.github.comment(1, {
    login: "codex[bot]",
    path: "src/login.rs",
    line: 1,
    body: [
      "**<sub><sub>![P2 Badge](https://img.shields.io/badge/P2-yellow?style=flat)</sub></sub>  Hash the password**",
      "",
      "Use `argon2` here; see [the guide](https://example.com/guide).",
      "![P0 Badge](https://img.shields.io/badge/P0-red?style=flat) ![diagram](https://example.com/d.png)",
      "A [relative link](../blob/main/x.rs) and <img src=x onerror=alert(1)>.",
      "",
      "<script>window.ran = true</script>",
      "<!-- codex-code-review-finding:v1 -->",
    ].join("\n"),
  });
  await page.goto(`${github.hub.url}#/items/app%231`);
  const comment = page
    .getByRole("region", { name: "app#1" })
    .getByRole("article", { name: "Comment c_1" });

  await expect(comment.locator("strong")).toContainText("Hash the password");
  await expect(comment.locator("code", { hasText: "argon2" })).toBeVisible();
  await expect(
    comment.getByRole("link", { name: "the guide" }),
  ).toHaveAttribute("target", "_blank");
  // The CSP loads no outside image: a shields.io badge is drawn in its
  // colour, louder for a higher priority, and any other image is a link.
  const p2 = comment.getByRole("link", { name: "P2", exact: true });
  await expect(p2).toHaveAttribute("href", /^https:\/\/img\.shields\.io\//);
  await expect(p2).toHaveAttribute("data-tone", "warning");
  await expect(p2).not.toHaveAttribute("data-solid");
  const p0 = comment.getByRole("link", { name: "P0", exact: true });
  await expect(p0).toHaveAttribute("data-tone", "error");
  await expect(p0).toHaveAttribute("data-solid");
  await expect(comment.getByRole("link", { name: "diagram" })).toHaveAttribute(
    "href",
    "https://example.com/d.png",
  );
  await expect(comment.getByText("relative link")).toBeVisible();
  await expect(
    comment.getByRole("link", { name: "relative link" }),
  ).toHaveCount(0);
  await expect(comment.locator("img, script")).toHaveCount(0);
  await expect(comment).not.toContainText("codex-code-review-finding");
  await expect(comment).not.toContainText("**");
  expect(await page.evaluate(() => "ran" in window)).toBe(false);

  // The Conversation list's one-line excerpt is the rendered text.
  await page.getByRole("button", { name: /^Conversation/ }).click();
  await expect(page.getByRole("listitem", { name: "c_1" })).toContainText(
    "P2 Hash the password Use argon2 here",
  );
});
