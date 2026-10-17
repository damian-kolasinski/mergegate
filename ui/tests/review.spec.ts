// Viewing an item: open an item, see every changed file in one
// column, walk it with `n/p`, `j/k` and `]/[`, expand the gaps between hunks.

import { chmodSync, mkdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import type { ItemDiff } from "../api/ItemDiff";
import { expect, git, type Hub, test } from "./hub";
import { clickToFrame, pressToFrame, report } from "./timing";

const lines = (count: number, line = (n: number) => `line ${n}`) =>
  Array.from({ length: count }, (_, i) => `${line(i + 1)}\n`).join("");

/**
 * app#1: `a.txt` changes lines 10 and 50, leaving 33 lines between its two
 * hunks; `gone.txt` is deleted, `blob.bin` is binary, `new.txt` is new.
 * The heuristics rank the text files low as docs, so `blob.bin` comes first.
 */
function submitChange(hub: Hub) {
  const app = hub.project("app");
  app.seed({ "a.txt": lines(60), "gone.txt": "bye\n" });
  app.submit("work", "Change things", {
    "a.txt": lines(60, (n) =>
      n === 10 || n === 50 ? `changed ${n}` : `line ${n}`,
    ),
    "gone.txt": null,
    "blob.bin": Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 0, 0, 1]),
    "new.txt": "hello\n",
  });
}

test("f6 an item shows every changed file", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByRole("heading")).toHaveText("Change things");
  const files = screen.getByRole("navigation", { name: "Files" });
  await expect(files.getByRole("listitem")).toHaveText([
    /^blob\.bin$/,
    /^a\.txt\s*ranked low: docs\s*\+2\s*−2$/,
    /^gone\.txt\s*ranked low: docs\s*−1$/,
    /^new\.txt\s*ranked low: docs\s*\+1$/,
  ]);

  const diff = screen.getByRole("region", { name: "Diff" });
  for (const text of ["changed 10", "changed 50", "bye", "hello"]) {
    await expect(diff.getByText(text, { exact: true })).toBeVisible();
  }
  await expect(diff.getByText("Binary file not shown")).toBeVisible();
  // Three lines of context, the rest folded.
  await expect(diff.getByText("line 7", { exact: true })).toBeVisible();
  await expect(diff.getByText("line 6", { exact: true })).toHaveCount(0);
});

test("f6 n and p move between files", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const current = page
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  // The diff reports the scroll a key caused a frame later; the next key
  // waits for it, as the operator would.
  const press = async (key: string, file: string) => {
    await page.keyboard.press(key);
    await expect(current).toContainText(file);
    await page.evaluate(
      () =>
        new Promise((done) =>
          requestAnimationFrame(() => requestAnimationFrame(done)),
        ),
    );
  };
  await expect(current).toContainText("blob.bin");
  await press("n", "a.txt");
  await press("n", "gone.txt");
  await press("n", "new.txt");
  // The last file stays current.
  await press("n", "new.txt");
  await press("p", "gone.txt");
});

test("f6 j, k, ] and [ move the line cursor", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  // The line's content cell; its gutter carries the mark too.
  const cursor = page.locator("[data-line][data-editor-active-line]");
  await expect(page.getByText("changed 10", { exact: true })).toBeVisible();
  await expect(cursor).toHaveCount(0);
  await page.keyboard.press("j");
  await expect(cursor).toHaveText("line 7");
  await page.keyboard.press("j");
  await expect(cursor).toHaveText("line 8");
  await page.keyboard.press("k");
  await expect(cursor).toHaveText("line 7");

  await page.keyboard.press("]");
  await expect(cursor).toHaveText("line 47");
  // The next hunk is the next file's.
  await page.keyboard.press("]");
  await expect(cursor).toHaveText("bye");
  // The diff reports the scroll that showed the line a frame later; the file
  // moved to stays current past it.
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
  await expect(
    page.getByRole("navigation", { name: "Files" }).locator("[aria-current]"),
  ).toContainText("gone.txt");
  await page.keyboard.press("[");
  await page.keyboard.press("[");
  await expect(cursor).toHaveText("line 7");

  // `n` puts the cursor on the file's first line.
  await page.keyboard.press("n");
  await expect(cursor).toHaveText("bye");
});

test("f6 j walks a realigned change in the order it shows", async ({
  page,
  hub,
}) => {
  // The view lines the removed call up with its edit, after the two added
  // lines, rather than next to the first added one.
  const app = hub.project("app");
  app.seed({ "call.rs": "keep\nfoo(1, 2)\nkeep2\n" });
  app.submit("work", "Edit a call", {
    "call.rs": "keep\nbar\nbaz\nfoo(1, 3)\nkeep2\n",
  });
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.getByText("foo(1, 3)", { exact: true })).toBeVisible();

  const cursor = page.locator("[data-line][data-editor-active-line]");
  for (const text of ["keep", "bar", "baz", "foo(1, 2)", "foo(1, 3)"]) {
    await page.keyboard.press("j");
    await expect(cursor).toHaveText(text);
  }
});

test("f6 a gap between hunks expands", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const line = page.getByText("line 30", { exact: true });
  await expect(line).toHaveCount(0);
  await page
    .locator("[data-separator]", { hasText: "33 unmodified lines" })
    .getByRole("button")
    .first()
    .click();
  await expect(line).toBeVisible();
});

test("f6 a hunk that holds the whole file offers nothing past it", async ({
  page,
  hub,
}) => {
  // Each hunk ends on three lines of context: short.txt's are its last.
  const app = hub.project("app");
  app.seed({ "short.txt": lines(7), "long.txt": lines(20) });
  app.submit("work", "Change both", {
    "short.txt": lines(7, (n) => (n === 4 ? "short" : `line ${n}`)),
    "long.txt": lines(20, (n) => (n === 4 ? "long" : `line ${n}`)),
  });
  await page.goto(`${hub.url}#/items/app%231`);

  const file = (text: string) => page.locator("code").filter({ hasText: text });
  const more = "More unchanged context may be available";
  await expect(file("long").getByText(more).first()).toBeVisible();
  await expect(file("short").getByText("line 7")).toBeVisible();
  await expect(file("short").getByText(more)).toHaveCount(0);
});

test("f6 v marks the current file viewed and moves on", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const screen = page.getByRole("region", { name: "app#1" });
  const current = screen
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  const changed = page.getByText("changed 10", { exact: true });
  await expect(changed).toBeVisible();
  await page.keyboard.press("n");
  await expect(current).toContainText("a.txt");
  // `n`'s scroll is reported a frame later, and would move the current
  // file back to a.txt after `v` moved on.
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
  await page.keyboard.press("v");
  await expect(current).toContainText("gone.txt");
  await expect(screen.getByLabel("Viewed a.txt")).toBeChecked();
  await expect(screen.getByText("1 of 4 reviewed")).toBeVisible();
  // Collapsed to its header, and out of the cursor's way.
  await expect(changed).toHaveCount(0);
  const cursor = page.locator("[data-line][data-editor-active-line]");
  await expect(cursor).toHaveText("bye");
  await page.keyboard.press("k");
  await expect(cursor).toHaveText("bye");

  // The hub keeps it.
  await page.reload();
  await expect(screen.getByLabel("Viewed a.txt")).toBeChecked();
  await expect(page.getByText("bye", { exact: true })).toBeVisible();
  await expect(changed).toHaveCount(0);

  await screen.getByRole("listitem").filter({ hasText: "a.txt" }).click();
  await page.keyboard.press("v");
  await expect(screen.getByLabel("Viewed a.txt")).not.toBeChecked();
  await expect(changed).toBeVisible();
  await expect(current).toContainText("a.txt");
  await expect(screen.getByText("0 of 4 reviewed")).toBeVisible();
});

test("f6 v right after moving to a file still moves on", async ({
  page,
  hub,
}) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  const current = screen
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  await expect(page.getByText("changed 10", { exact: true })).toBeVisible();

  // `v` lands after the view scrolled where `n` asked, in the frame after,
  // but before the browser reports that scroll.
  await page.evaluate(() => {
    const press = (key: string) =>
      window.dispatchEvent(new KeyboardEvent("keydown", { key }));
    press("n");
    requestAnimationFrame(() => press("v"));
  });
  await expect(screen.getByLabel("Viewed a.txt")).toBeChecked();
  await page.evaluate(
    () =>
      new Promise((done) =>
        requestAnimationFrame(() => requestAnimationFrame(done)),
      ),
  );
  await expect(current).toContainText("gone.txt");
});

test("f6 a file's header marks it viewed", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);

  const files = page.getByRole("navigation", { name: "Files" });
  const diff = page.getByRole("region", { name: "Diff" });
  // The list only shows the mark; the diff's header is where it's made.
  await expect(files.getByRole("checkbox")).toHaveCount(0);
  await diff.getByText("new.txt", { exact: true }).click();
  await expect(page.getByText("1 of 4 reviewed")).toBeVisible();
  await expect(page.getByText("hello", { exact: true })).toHaveCount(0);
  await expect(diff.getByLabel("Viewed new.txt")).toBeChecked();
  // The header doesn't keep the keys.
  await page.keyboard.press("n");
  await expect(files.locator("li[aria-current=true]")).toContainText("a.txt");

  await page.reload();
  // Its box unmarks it.
  await diff.getByLabel("Viewed new.txt").uncheck();
  await expect(page.getByText("0 of 4 reviewed")).toBeVisible();
  await expect(page.getByText("hello", { exact: true })).toBeVisible();
});

test("f6 escape goes back to the inbox", async ({ page, hub }) => {
  submitChange(hub);
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.getByText("changed 10", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("region", { name: "Needs you" })).toBeVisible();
});

// Budget: a 5k-line diff scrolls at 60 fps. Measured everywhere,
// enforced with MG_ENFORCE_BUDGETS=1, which CI sets on Ubuntu only.
test("a 5k-line diff scrolls within budget", async ({ page, hub }, info) => {
  const files: Record<string, string> = {};
  for (let file = 1; file <= 50; file++) {
    files[`src/module_${file}.rs`] = lines(
      100,
      (n) => `    let value_${n} = compute(${file}, ${n}).unwrap_or_default();`,
    );
  }
  hub.project("big").submit("big", "A large change", files);
  await page.goto(`${hub.url}#/items/big%231`);
  const diff = page.getByRole("region", { name: "Diff" });
  await expect(
    diff.getByText("let value_1 =", { exact: false }).first(),
  ).toBeVisible();

  // A fast fling, ten lines a frame from top to bottom, so CodeView renders
  // fresh rows every frame; the time between frames is what the reviewer
  // would see.
  const frames = await diff.evaluate(
    (view) =>
      new Promise<number[]>((resolve) => {
        const gaps: number[] = [];
        let last = performance.now();
        const step = (now: number) => {
          gaps.push(now - last);
          last = now;
          view.scrollTop += 200;
          const end = view.scrollTop + view.clientHeight >= view.scrollHeight;
          if (end || gaps.length > 2000) resolve(gaps.slice(1));
          else requestAnimationFrame(step);
        };
        requestAnimationFrame(step);
      }),
  );
  const sorted = frames.sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.floor((sorted.length - 1) * q)] ?? 0;
  info.annotations.push({
    type: "scroll frames",
    description: `p50 ${at(0.5).toFixed(1)} ms, p95 ${at(0.95).toFixed(1)} ms, max ${at(1).toFixed(1)} ms over ${sorted.length} frames`,
  });
  expect(sorted.length).toBeGreaterThan(100);
  // A frame dropped at 60 fps takes two refreshes, 33 ms.
  if (process.env.MG_ENFORCE_BUDGETS === "1") expect(at(0.95)).toBeLessThan(25);
});

// Budget: opening an item from the inbox shows its diff within
// 100 ms, and moving through it (`n`, `j`, `v`) shows by the frame after the
// next.
test("a large item opens and answers keys within budget", async ({
  page,
  hub,
}, info) => {
  const big = hub.project("big");
  const items = 5;
  for (let item = 1; item <= items; item++) {
    const files: Record<string, string> = {};
    for (let file = 1; file <= 50; file++) {
      files[`src/module_${file}.rs`] = lines(
        100,
        (n) => `    let value_${n} = compute(${item}, ${file}, ${n});`,
      );
    }
    big.submit(`big-${item}`, `Large change ${item}`, files);
  }
  await page.goto(hub.url);
  const inbox = page.getByRole("region", { name: "Needs you" });
  await expect(inbox.getByRole("listitem")).toHaveCount(items);

  // Each item opened once, as the operator meets it.
  const diffShown = { selector: "[data-line]", read: "found" } as const;
  const opened: number[] = [];
  for (let item = 0; item < items; item++) {
    opened.push(await pressToFrame(page, "Enter", diffShown));
    await page.keyboard.press("Escape");
    await expect(inbox).toBeVisible();
    await page.keyboard.press("j");
  }
  report(info, "open an item", opened, 100);

  await page.keyboard.press("Enter");
  const files = page.getByRole("navigation", { name: "Files" });
  await expect(files.locator("li[aria-current=true]")).toBeVisible();
  const probes = {
    n: {
      selector: '[aria-label="Files"] li[aria-current="true"]',
      read: "text",
    },
    j: { selector: "[data-line][data-editor-active-line]", read: "text" },
    v: {
      selector: '[aria-label="Files"] [role="img"][aria-label="Viewed"]',
      read: "count",
    },
  } as const;
  const keys = { n: [] as number[], j: [] as number[], v: [] as number[] };
  for (const [key, steps] of [
    ["n", 15],
    ["j", 30],
    ["v", 15],
  ] as const) {
    for (let step = 0; step < steps; step++) {
      // Lets the last step's scrolling settle, as between real presses.
      await page.waitForTimeout(50);
      keys[key].push(await pressToFrame(page, key, probes[key]));
    }
  }
  // Timed to the frame painted, so the wait for the display's next refresh
  // counts: one refresh of work at most leaves the result two refreshes
  // after the press.
  report(info, "next file (n)", keys.n, 33);
  report(info, "next line (j)", keys.j, 33);
  report(info, "mark viewed (v)", keys.v, 33);
});

/** A pack built by `langs/build.mjs`, as highlight.spec.ts installs. */
const jsonPack = resolve(
  import.meta.dirname,
  "../../crates/highlight/tests/fixtures/json-0.24.8.tar.gz",
);

/**
 * Waits for the hub's spans on every file of item `id`. It highlights in
 * the background after a submit, and opening a file before then would time
 * that pass, not the open.
 */
async function highlighted(hub: Hub, id: string) {
  const route = `${hub.url}api/items/${encodeURIComponent(id)}`;
  await expect
    .poll(async () => {
      const diff = (await (await fetch(route)).json()) as ItemDiff;
      return diff.files.every(
        (file) =>
          file.content.kind === "text" &&
          file.content.hunks.some((hunk) => hunk.spans.some((s) => s.length)),
      );
    })
    .toBe(true);
}

/** A JSON object of `count` entries, one a line after the opening brace. */
const json = (count: number, entry: (n: number) => string) =>
  `{\n${lines(count, entry)}  "end": 0\n}\n`;

// Budget: the largest file the hub keeps hunks for opens within
// 150 ms, highlighted by an installed pack: 3.6× the lines of the 100 ms
// item, all in one file, which renders whole.
test("a single large file opens within budget", async ({ page, hub }, info) => {
  hub.mg(["lang", "add", "--from", jsonPack]);
  const big = hub.project("big");
  // 18k changed lines, near the 20k past which a file is too large to show.
  const size = 9000;
  big.seed({ "big.json": json(size, (n) => `  "old_${n}": ${n},`) });
  const items = 5;
  for (let item = 1; item <= items; item++) {
    big.submit(`big-${item}`, `Rewrite ${item}`, {
      "big.json": json(size, (n) => `  "new_${item}_${n}": ${n},`),
    });
  }
  for (let item = 1; item <= items; item++)
    await highlighted(hub, `big#${item}`);
  await page.goto(hub.url);
  const inbox = page.getByRole("region", { name: "Needs you" });
  await expect(inbox.getByRole("listitem")).toHaveCount(items);

  // Timed to the hub's spans painted, not to a plain render before them.
  const painted = { selector: "[data-tok]", read: "found" } as const;
  const opened: number[] = [];
  for (let item = 0; item < items; item++) {
    opened.push(await pressToFrame(page, "Enter", painted));
    await page.keyboard.press("Escape");
    await expect(inbox).toBeVisible();
    await page.keyboard.press("j");
  }
  report(info, "open a single large file", opened, 150);
});

// Budget: expanding a gap between hunks shows its lines within
// 50 ms, the file's blobs and their spans fetched from the hub included.
// Every file's gaps are long and its rows virtualized as the 5k-line diff's.
test("a gap expands within budget", async ({ page, hub }, info) => {
  hub.mg(["lang", "add", "--from", jsonPack]);
  const files: Record<string, string> = {};
  const changed: Record<string, string> = {};
  const count = 10;
  for (let file = 1; file <= count; file++) {
    const path = `gap_${file}.json`;
    const entry = (n: number) => `  "value_${n}": ${file * n},`;
    files[path] = json(2000, entry);
    changed[path] = json(2000, (n) =>
      n === 100 || n === 1900 ? `  "changed_${n}": 0,` : entry(n),
    );
  }
  const app = hub.project("app");
  app.seed(files);
  app.submit("gaps", "Change two entries a file", changed);
  await highlighted(hub, "app#1");
  await page.goto(`${hub.url}#/items/app%231`);
  await expect(page.locator("[data-tok]").first()).toBeVisible();

  // The gap between each file's hunks, expanded by the step its buttons
  // show, 20 lines. Every one is the first its file expands, so each
  // fetches the file's blobs and their spans.
  const gap = page
    .locator("[data-separator]", { hasText: "1793 unmodified lines" })
    .first()
    .locator("[data-expand-down]");
  const gaps = { selector: "[data-unmodified-lines]", read: "texts" } as const;
  const current = page
    .getByRole("navigation", { name: "Files" })
    .locator("li[aria-current=true]");
  const expanded: number[] = [];
  for (let file = 1; file <= count; file++) {
    if (file > 1) {
      const was = await current.textContent();
      await page.keyboard.press("n");
      await expect(current).not.toHaveText(was ?? "");
    }
    await expect(gap).toBeVisible();
    await page.waitForTimeout(50);
    expanded.push(await clickToFrame(page, gap, gaps));
  }
  report(info, "expand a gap", expanded, 50);
});

test("f6 a mode or type change says so in the file's header", async ({
  page,
  hub,
}) => {
  const app = hub.project("app");
  app.seed({ "run.sh": "echo hi\n" });
  git(app.dir, "switch", "--quiet", "-c", "modes", "main");
  chmodSync(join(app.dir, "run.sh"), 0o755);
  symlinkSync("/etc/passwd", join(app.dir, "evil"));
  // A gitlink needs no commit behind it, and its empty directory is an
  // unpopulated submodule to `git add`, not a deletion.
  mkdirSync(join(app.dir, "dep"));
  const pinned = "1".repeat(40);
  git(app.dir, "update-index", "--add", "--cacheinfo", `160000,${pinned},dep`);
  app.revise("Modes", {});
  await page.goto(`${hub.url}#/items/app%231`);

  const diff = page.getByRole("region", { name: "Diff" });
  // Only the mode changed, so the header is all there is to see.
  await expect(
    diff.getByText("Made executable (mode 100644 → 100755)"),
  ).toBeVisible();
  await expect(diff.getByText("Symlink", { exact: true })).toBeVisible();
  await expect(diff.getByText("/etc/passwd", { exact: true })).toBeVisible();
  await expect(diff.getByText("Submodule at 1111111")).toBeVisible();
  await expect(diff.getByText("Binary file not shown")).toHaveCount(0);
});
