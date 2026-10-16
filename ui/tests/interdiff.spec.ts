// F7 from the UI: a revision submitted mid-review waits behind a banner, and
// switching to it shows only what changed since the revision reviewed, over
// a target that moved meanwhile.

import type { Page } from "@playwright/test";
import { expect, git, type Hub, test } from "./hub";

/**
 * app#1 r1 adds `login.rs` and changes `lib.rs`; the operator views
 * `login.rs` and requests changes. Returns a step that has the agent rebase
 * onto a `main` whose README moved and change `lib.rs` again, as r2.
 */
function reviewed(hub: Hub) {
  const app = hub.project("app");
  app.seed({ README: "title\n", "src/lib.rs": "lib\n" });
  app.submit("login", "Add login", {
    "src/lib.rs": "lib\nmod login;\n",
    "src/login.rs": "fn login() {}\n",
  });
  return () => {
    app.seed({ README: "title\nmore\n" });
    git(app.dir, "switch", "--quiet", "login");
    git(app.dir, "rebase", "--quiet", "main");
    app.revise("Add tokens", { "src/lib.rs": "lib\nmod login;\nmod token;\n" });
  };
}

async function requestChanges(page: Page, revision = 1) {
  const review = page.getByRole("form", { name: "Review" });
  await review.getByRole("textbox", { name: "Review summary" }).fill("Fix it.");
  await review.getByRole("button", { name: "Request changes" }).click();
  await expect(
    review.getByText(`Sent to the agent on r${revision}`),
  ).toBeVisible();
}

test("f7 a new revision shows what changed since the review", async ({
  page,
  hub,
}) => {
  const rebase = reviewed(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const files = screen.getByRole("navigation", { name: "Files" });
  const diff = screen.getByRole("region", { name: "Diff" });
  const review = page.getByRole("form", { name: "Review" });
  const current = files.locator("li[aria-current=true]");
  // It adds a declaration, which ranks it first.
  await expect(current).toContainText("login.rs");
  await diff.getByLabel("Viewed src/login.rs").check();
  await requestChanges(page);

  // r2 waits behind a banner, and only it can be approved.
  rebase();
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await expect(banner.getByText("r2 submitted")).toBeVisible();
  await expect(diff.getByText("mod token;")).toHaveCount(0);
  await expect(
    review.getByRole("button", { name: "Approve & merge" }),
  ).toBeDisabled();

  // What's typed survives the switch and comparing another way.
  const summary = review.getByRole("textbox", { name: "Review summary" });
  await summary.fill("Closer.");
  await banner.getByRole("button", { name: "Show r2" }).click();
  await expect(banner).toHaveCount(0);
  await expect(summary).toHaveValue("Closer.");
  // r2 opens on all its changes, and Compare narrows them to r1's.
  await expect(screen.getByLabel("Compare")).toHaveValue("all");
  await expect(files.getByRole("listitem")).toHaveText([
    /^login\.rs\s*src/,
    /^lib\.rs\s*src/,
  ]);
  await screen.getByLabel("Compare").selectOption("1");
  await expect(summary).toHaveValue("Closer.");
  // main's README isn't the item's, and login.rs is as r1 left it, its
  // mark with it.
  await expect(files.getByRole("listitem")).toHaveText([/^lib\.rs\s*src/]);
  // login.rs was current, and isn't shown now.
  await expect(current).toContainText("lib.rs");
  await expect(diff.getByText("mod token;")).toBeVisible();
  await expect(diff.getByText("more", { exact: true })).toHaveCount(0);
  await files.getByText("1 unchanged since r1").click();
  await expect(files.getByLabel("Viewed src/login.rs")).toBeChecked();
  await expect(screen.getByText("1 of 2 reviewed")).toBeVisible();
  await expect(
    review.getByRole("button", { name: "Approve & merge" }),
  ).toBeEnabled();

  await screen.getByLabel("Compare").selectOption("all");
  await expect(files.getByRole("listitem")).toHaveText([
    /^login\.rs\s*src/,
    /^lib\.rs\s*src/,
  ]);
  await expect(files.getByText("unchanged since")).toHaveCount(0);
  await expect(summary).toHaveValue("Closer.");
  // lib.rs is shown again with its changes against the base: `mod login;`
  // is r1's addition, context since r1.
  const added = diff.locator('[data-line-type="change-addition"]');
  await expect(added.filter({ hasText: /^mod login;$/ })).toHaveCount(1);
  await expect(screen.getByLabel("Compare")).toHaveValue("all");

  // The picker says what the diff is against, though a new diff, as a
  // refetch brings, renders its options anew.
  const compare = screen.getByLabel("Compare");
  await compare.selectOption("1");
  await expect(files.getByText("1 unchanged since r1")).toBeVisible();
  await expect(compare).toHaveValue("1");
  const refetched = page.waitForResponse((r) => r.url().endsWith("?since=1"));
  await compare.selectOption("1");
  await refetched;
  await expect(compare).toHaveValue("1");
});

test("f7 the inbox counts what changed since the review", async ({
  page,
  hub,
}) => {
  const rebase = reviewed(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await requestChanges(page);
  rebase();

  await page.goto(hub.url);
  await expect(page.getByRole("listitem")).toContainText("1 changed since r1");

  // Opening it starts on all changes.
  await page.keyboard.press("Enter");
  await expect(page.getByLabel("Compare")).toHaveValue("all");
});

test("f7 a file new since the review says so", async ({ page, hub }) => {
  const app = hub.project("app");
  app.seed({ "src/lib.rs": "lib\n" });
  app.submit("login", "Add login", { "src/lib.rs": "lib\nmod login;\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  await requestChanges(page);
  app.revise("Add tokens", { "src/token.rs": "fn token() {}\n" });

  const screen = page.getByRole("region", { name: "app#1" });
  const banner = screen.getByRole("region", { name: "Newer revision" });
  await banner.getByRole("button", { name: "Show r2" }).click();
  await screen.getByLabel("Compare").selectOption("1");
  const files = screen.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^token\.rs\s*src.*added/,
  ]);
  const diff = screen.getByRole("region", { name: "Diff" });
  await expect(
    diff.getByText("Added since the earlier revision"),
  ).toBeVisible();
});

test("f7 an unchanged file keeps a thread that waits in the diff", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ README: "title\n" });
  app.submit("greet", "Add greet", { "src/greet.py": "def greet(): pass\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const files = screen.getByRole("navigation", { name: "Files" });
  const diff = screen.getByRole("region", { name: "Diff" });
  await expect(diff.getByText("def greet(): pass")).toBeVisible();
  await page.keyboard.press("j");
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on src/greet.py line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Add a test.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await diff.getByLabel("Viewed src/greet.py").check();
  await requestChanges(page);
  hub.mg(["reply", "c_1", "--status", "addressed"]);
  app.revise("Test greet", { "tests/test_greet.py": "greet()\n" });

  // r2 leaves greet.py as it was, but the agent answered its comment: the
  // file stays in the diff, open over its viewed mark, with the thread.
  await page.reload();
  await screen.getByLabel("Compare").selectOption("1");
  await expect(files.getByRole("listitem")).toHaveText([
    /^greet\.py\s*src.*unchanged since r1/,
    /^test_greet\.py\s*tests.*added/,
  ]);
  await expect(files.getByText(/\d unchanged since/)).toHaveCount(0);
  await expect(diff.getByLabel("Viewed src/greet.py")).toBeChecked();
  const comment = diff.getByRole("article", { name: "Comment c_1" });
  await expect(comment).toContainText("Add a test.");
  await expect(comment).toContainText("Addressed in r2");

  // Reviewed on r2, the thread is settled: r3 folds the file away again,
  // and the Conversation list holds the thread.
  await requestChanges(page, 2);
  app.revise("Name it", { "src/name.py": "NAME = 'you'\n" });
  await page.reload();
  await screen.getByLabel("Compare").selectOption("2");
  await expect(files.getByRole("listitem")).toHaveText([/^name\.py\s*src/]);
  await expect(files.getByText("2 unchanged since r2")).toBeVisible();
  await expect(comment).toHaveCount(0);
  await screen.getByRole("button", { name: /^Conversation/ }).click();
  const list = page.getByRole("complementary", { name: "Conversation" });
  await expect(list.getByRole("button", { name: "Show in diff" })).toHaveCount(
    0,
  );
  await expect(
    list.getByRole("article", { name: "Comment c_1" }),
  ).toContainText("Addressed in r2");
});

test("f7 an unchanged file stays in the diff once its thread is settled", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "src/greet.py": "def greet(): return None\n" });
  app.submit("greet", "Add greet", { "src/greet.py": "def greet(): pass\n" });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const files = screen.getByRole("navigation", { name: "Files" });
  const diff = screen.getByRole("region", { name: "Diff" });
  const review = page.getByRole("form", { name: "Review" });
  const current = files.locator("li[aria-current=true]");
  await expect(diff.getByText("def greet(): pass")).toBeVisible();
  // On the removed line, which an interdiff's replayed old side lacks.
  await page.keyboard.press("j");
  await page.keyboard.press("c");
  await expect(
    page.getByRole("textbox", { name: "Comment on src/greet.py line 1" }),
  ).toBeFocused();
  await page.keyboard.type("Why not None?");
  await page.keyboard.press("ControlOrMeta+Enter");
  await requestChanges(page);
  app.revise("Test greet", { "tests/test_greet.py": "greet()\n" });
  await page.reload();
  await requestChanges(page, 2);
  hub.mg(["reply", "c_1", "--status", "question", "--body", "Which inputs?"]);
  app.revise("Test more", { "tests/test_more.py": "greet()\n" });

  // greet.py is as r2 left it, but the agent asked on its removed line: the
  // file is in the diff, its own base on the left.
  await page.reload();
  await screen.getByLabel("Compare").selectOption("2");
  await expect(files.getByRole("listitem")).toHaveText([
    /^greet\.py\s*src.*unchanged since r2/,
    /^test_more\.py\s*tests.*added/,
  ]);
  await expect(current).toContainText("greet.py");
  const comment = diff.getByRole("article", { name: "Comment c_1" });
  await expect(comment).toContainText("Which inputs?");

  // Answering settles the thread; the file, the thread and the operator's
  // place stay.
  await comment.getByRole("button", { name: "Reply" }).click();
  await expect(
    comment.getByRole("textbox", { name: "Reply to c_1" }),
  ).toBeFocused();
  await page.keyboard.type("Any name.");
  await page.keyboard.press("ControlOrMeta+Enter");
  await review.getByRole("button", { name: "Comment" }).click();
  await expect(
    review.getByText("Sent to the agent on r3 with 1 reply"),
  ).toBeVisible();
  await expect(files.getByRole("listitem")).toHaveText([
    /^greet\.py\s*src.*unchanged since r2/,
    /^test_more\.py\s*tests.*added/,
  ]);
  await expect(current).toContainText("greet.py");
  await expect(comment).toContainText("Any name.");
  await expect(comment.getByText("Draft")).toHaveCount(0);
});
