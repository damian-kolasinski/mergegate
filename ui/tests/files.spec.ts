// The file list as a tree: each module the hub found in the repository,
// then its folders, then its files, with a test under the file it covers
// and why a file is here behind its ⓘ.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "./hub";

const GUIDE = `Lets a group be renamed.

## Files

- \`Products/Messenger/ChatGroups/ChatGroups/Scenes/RenameView.swift\` — the new screen, start here
- \`Products/Messenger/Profile/Profile/Avatar.swift\` — shows the new name
- \`Products/Messenger/ChatGroups/ChatGroupsInterface/ChatGroupsRoute.swift\` — the route to it
- \`Products/Messenger/ChatGroups/ChatGroupsTests/Scenes/RenameViewTests.swift\` — covers it
`;

test("f11 the list groups files by the Xcode target groups they're in", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({
    "Products/Messenger/ChatGroups/Project.swift": "let project = 1\n",
    "Products/Messenger/Profile/Profile/Profile.swift": "struct Profile {}\n",
  });
  app.submit("rename", "Rename a group", {
    "Products/Messenger/ChatGroups/ChatGroups/Scenes/RenameView.swift":
      "struct RenameView {}\n",
    "Products/Messenger/ChatGroups/ChatGroupsTests/Scenes/RenameViewTests.swift":
      "final class RenameViewTests {}\n",
    "Products/Messenger/ChatGroups/ChatGroupsInterface/ChatGroupsRoute.swift":
      "enum ChatGroupsRoute { case rename }\n",
    "Products/Messenger/Profile/Profile/Avatar.swift": "struct Avatar {}\n",
    "README.md": "# app\n",
  });
  const guide = join(hub.root, "guide.md");
  writeFileSync(guide, GUIDE);
  hub.mg(["submit", "--guide", guide], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  const chatGroups = files.getByRole("group", { name: "ChatGroups" });
  // Where the module lives, said once on it.
  await expect(
    chatGroups.getByRole("button", { name: /^ChatGroups/ }),
  ).toContainText("Products/Messenger");
  // The test hangs off the screen it covers, the interface is tagged, and
  // a folder in no module stays a folder.
  await expect(files.getByRole("listitem")).toHaveText([
    /^RenameView\.swift\s*\+1$/,
    /^RenameViewTests\.swift\s*\+1$/,
    /^ChatGroupsRoute\.swift\s*Interface\s*\+1$/,
    /^Avatar\.swift\s*\+1$/,
    /^README\.md\s*\+1$/,
  ]);
  await expect(chatGroups.getByRole("group", { name: "Scenes" })).toBeVisible();
  await expect(files.getByRole("group", { name: "Profile" })).toBeVisible();
  // The diff reads down the list rather than in the guide's order.
  const diff = page.getByRole("region", { name: "Diff" });
  await expect
    .poll(() =>
      diff
        .getByRole("checkbox", { name: /^Viewed / })
        .evaluateAll((boxes) =>
          boxes.map((box) => box.getAttribute("aria-label")),
        ),
    )
    .toEqual([
      "Viewed Products/Messenger/ChatGroups/ChatGroups/Scenes/RenameView.swift",
      "Viewed Products/Messenger/ChatGroups/ChatGroupsTests/Scenes/RenameViewTests.swift",
      "Viewed Products/Messenger/ChatGroups/ChatGroupsInterface/ChatGroupsRoute.swift",
      "Viewed Products/Messenger/Profile/Profile/Avatar.swift",
      "Viewed README.md",
    ]);

  // Why a file is here waits behind its ⓘ, beside the list.
  const note = page.getByRole("dialog", { name: "Why this file" });
  await expect(note).toHaveCount(0);
  await files
    .getByRole("button", {
      name: "Why Products/Messenger/Profile/Profile/Avatar.swift is here",
    })
    .click();
  await expect(note).toContainText("shows the new name");
  const current = files.locator("li[aria-current=true]");
  await expect(current).toContainText("Avatar.swift");
  // `Escape` closes it rather than leaving the item.
  await page.keyboard.press("Escape");
  await expect(note).toHaveCount(0);
  await expect(page.getByRole("region", { name: "app#1" })).toBeVisible();

  // `i` opens it for the current file, and it follows `p` up the list.
  await page.keyboard.press("i");
  await expect(note).toContainText("shows the new name");
  await page.keyboard.press("p");
  await expect(current).toContainText("ChatGroupsRoute.swift");
  await expect(note).toContainText("the route to it");
  await page.keyboard.press("i");
  await expect(note).toHaveCount(0);

  // A folded module hides its files until the cursor moves into it.
  const fold = chatGroups.getByRole("button", { name: /^ChatGroups/ });
  await fold.click();
  await expect(fold).toHaveAttribute("aria-expanded", "false");
  await expect(files.getByRole("listitem")).toHaveText([
    /^Avatar\.swift/,
    /^README\.md/,
  ]);
  await page.keyboard.press("p");
  await page.keyboard.press("p");
  await expect(fold).toHaveAttribute("aria-expanded", "true");
  await expect(current).toContainText("RenameView.swift");
});

test("f11 the list groups files by the crates they're in", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({
    "Cargo.toml": "[workspace]\n",
    "crates/hub/Cargo.toml": "[package]\n",
    "crates/core/Cargo.toml": "[package]\n",
    "web/package.json": "{}\n",
  });
  app.submit("routes", "Add a route", {
    "crates/hub/src/routes.rs": "pub fn routes() {}\n",
    "crates/hub/src/routes_tests.rs": "#[test]\nfn routes() {}\n",
    "crates/hub/tests/serve.rs": "#[test]\nfn serves() {}\n",
    "crates/core/src/item.rs": "pub struct Item;\n",
    "crates/core/tests/item.rs": "#[test]\nfn items() {}\n",
    "web/pages/home.ts": "export {};\n",
    "web/pages/about.ts": "export {};\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  const crate = files.getByRole("group", { name: "hub" });
  // The crate's `src` is its code, so its files sit right under it; its
  // `tests` are tagged.
  await expect(crate.getByRole("listitem")).toHaveText([
    /^routes\.rs/,
    /^routes_tests\.rs/,
    /^serve\.rs\s*Tests/,
  ]);
  await expect(crate.getByRole("button", { name: /^hub/ })).toContainText(
    "crates",
  );
  // A test in `tests` named like a file in `src` covers it.
  await expect(
    files.getByRole("group", { name: "core" }).getByRole("listitem"),
  ).toHaveText([/^item\.rs\s*\+1$/, /^item\.rs\s*\+2$/]);
  // A module at the top says only the folder its files share.
  await expect(files.getByRole("button", { name: /^web/ })).toContainText(
    /^web\s*pages\//,
  );
});

test("f11 a test hangs under the file of its own kind, and the diff follows", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.submit("button", "Add a button", {
    "web/button.test.ts": "test();\n",
    "web/button.css": ".button {}\n",
    "web/button.ts": "export {};\n",
  });
  // The test first, so the order the list and the diff are in differs from
  // the guide's.
  const guide = join(hub.root, "guide.md");
  writeFileSync(
    guide,
    `Adds a button.

## Files

- \`web/button.test.ts\` — start here
- \`web/button.css\` — its look
- \`web/button.ts\` — the button
`,
  );
  hub.mg(["submit", "--guide", guide], app.dir);
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^button\.ts/,
    /^button\.test\.ts/,
    /^button\.css/,
  ]);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect
    .poll(() =>
      diff
        .getByRole("checkbox", { name: /^Viewed / })
        .evaluateAll((boxes) =>
          boxes.map((box) => box.getAttribute("aria-label")),
        ),
    )
    .toEqual([
      "Viewed web/button.ts",
      "Viewed web/button.test.ts",
      "Viewed web/button.css",
    ]);
});
