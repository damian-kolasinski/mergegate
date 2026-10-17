// A recording for the stacked items PR: a stack submitted with
// `mg submit --on`, each part reviewed as its own diff, a child rebasing on
// `parent.updated`, and one merge landing the whole stack bottom first.
// Skipped unless MG_SHOTS_DIR names where to write it:
//
//   MG_SHOTS_DIR=$(mktemp -d) pnpm --dir ui e2e stack-demo --project chromium
//
// It writes stacked-items.mp4 and stacked-items.gif through ffmpeg, which
// has to be on PATH.

import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { expect, git, type Hub, type Project, test } from "./hub";

const out = process.env.MG_SHOTS_DIR ?? "";

test.skip(!out, "MG_SHOTS_DIR is unset");

const schema = `create table sessions (
  id text primary key,
  user_id integer not null references users (id),
  expires_at timestamp not null
);
`;

const schemaR2 = `create table sessions (
  id text primary key,
  user_id integer not null references users (id) on delete cascade,
  expires_at timestamp not null
);

create index sessions_by_user on sessions (user_id);
`;

const signIn = `import { db } from "../db";
import { randomToken } from "./token";

const week = 7 * 24 * 60 * 60 * 1000;

/** Opens a session for \`user\`, good for a week. */
export async function signIn(user: { id: number }): Promise<string> {
  const id = randomToken();
  await db.insert("sessions", {
    id,
    user_id: user.id,
    expires_at: new Date(Date.now() + week),
  });
  return id;
}
`;

const whoAmI = `import { db } from "../db";

/** The user a session cookie belongs to, unless it expired. */
export async function whoAmI(session: string) {
  const row = await db.one(
    "select users.* from sessions join users on users.id = sessions.user_id" +
      " where sessions.id = $1 and sessions.expires_at > now()",
    [session],
  );
  return row ?? null;
}
`;

/** Runs the commands of `item`'s newest `parent.updated` on `branch`. */
function rebase(hub: Hub, app: Project, item: string, branch: string) {
  const feedback = hub.mg(["feedback", item]) as {
    events: { type: string; commands?: string[] }[];
  };
  const updated = [...feedback.events]
    .reverse()
    .find((e) => e.type === "parent.updated");
  if (!updated?.commands) throw new Error(`${item} heard no parent.updated`);
  app.switch(branch);
  for (const command of updated.commands) {
    const [, ...args] = command.split(" ");
    git(app.dir, ...args);
  }
  hub.mg(["submit"], app.dir);
}

test("stacked items recording", async ({ browser, hub }, info) => {
  // Paced for a viewer, not for speed.
  test.setTimeout(120_000);
  const app = hub.project("app");
  app.seed({ "src/db.ts": "export const db = connect();\n" });
  app.submit("sessions-table", "Add a sessions table", {
    "migrations/0007_sessions.sql": schema,
  });
  app.stack("sign-in", "1", "Sign in with a session", {
    "src/auth/signIn.ts": signIn,
  });
  app.stack("who-am-i", "2", "Show who's signed in", {
    "src/auth/whoAmI.ts": whoAmI,
  });

  const videos = join(info.outputDir, "video");
  const context = await browser.newContext({
    viewport: { width: 1280, height: 800 },
    recordVideo: { dir: videos, size: { width: 1280, height: 800 } },
  });
  const page = await context.newPage();
  const pause = (ms: number) => page.waitForTimeout(ms);
  const caption = (text: string) =>
    page.evaluate((text) => {
      let bar = document.getElementById("demo-caption");
      if (!bar) {
        bar = document.createElement("div");
        bar.id = "demo-caption";
        bar.style.cssText =
          "position:fixed;left:50%;bottom:96px;transform:translateX(-50%);z-index:9999;" +
          "background:#0b1f3a;color:#fff;font:600 18px 'DM Sans Variable',system-ui;" +
          "padding:10px 18px;border-radius:10px;box-shadow:0 6px 24px rgb(0 0 0/.25)";
        document.body.append(bar);
      }
      bar.textContent = text;
    }, text);

  await page.goto(hub.url);
  const needsYou = page.getByRole("region", { name: "Needs you" });
  await expect(needsYou.getByText("↳ on app#2")).toBeVisible();
  await caption("The agent split its work into a stack: mg submit --on");
  await pause(3500);

  await needsYou.getByRole("link", { name: "Sign in with a session" }).click();
  const child = page.getByRole("region", { name: "app#2" });
  const stack = child.getByRole("navigation", { name: "Stack" });
  await expect(stack).toBeVisible();
  await caption("Each part is only its own commits, against the one below");
  await pause(4000);

  app.switch("sessions-table");
  app.revise("Cascade sessions with their user", {
    "migrations/0007_sessions.sql": schemaR2,
  });
  await expect(stack.getByText("behind")).toBeVisible();
  await caption("The bottom part gets a new revision: its child is behind");
  await pause(3500);

  rebase(hub, app, "app#2", "sign-in");
  rebase(hub, app, "app#3", "who-am-i");
  await caption("parent.updated gives the agent the rebase; it resubmits");
  await expect(child.getByText("r2 submitted")).toBeVisible();
  await pause(2500);
  await child.getByRole("button", { name: "Show r2" }).click();
  await expect(stack.getByText("behind")).toHaveCount(0);
  await pause(1200);
  await child.getByLabel("Compare").selectOption("1");
  await expect(child.getByText("Nothing changed since r1.")).toBeVisible();
  await caption("Its own diff is unchanged by the rebase");
  await pause(3500);

  await page
    .getByRole("button", { name: "Approve, merge after app#1" })
    .click();
  const confirm = page.getByRole("group", { name: "Confirm merge" });
  await pause(1200);
  await confirm.getByRole("button", { name: "Approve", exact: true }).click();
  await expect(
    child.getByRole("region", { name: "Merge waiting" }),
  ).toBeVisible();
  await caption("Approved parts wait for the one below to land");
  await pause(3000);

  await stack.getByRole("link", { name: /app#3/ }).click();
  const top = page.getByRole("region", { name: "app#3" });
  await expect(top.getByRole("navigation", { name: "Stack" })).toBeVisible();
  await page
    .getByRole("button", { name: "Approve, merge after app#2" })
    .click();
  await pause(800);
  await confirm.getByRole("button", { name: "Approve", exact: true }).click();
  await expect(
    top.getByRole("region", { name: "Merge waiting" }),
  ).toBeVisible();
  await pause(1500);

  await top
    .getByRole("navigation", { name: "Stack" })
    .getByRole("link", { name: /app#1/ })
    .click();
  const bottom = page.getByRole("region", { name: "app#1" });
  await expect(bottom.getByRole("navigation", { name: "Stack" })).toBeVisible();
  await caption("Merging the bottom lands the whole stack, bottom first");
  await pause(2500);
  await page.getByRole("button", { name: "Approve & merge" }).click();
  await pause(800);
  await confirm.getByRole("button", { name: "Merge" }).click();
  await expect(bottom.getByText(/Merged into main/)).toBeVisible();
  await pause(2500);

  await page.goto(hub.url);
  await page.getByText(/^Closed today/).click();
  await caption("Three squash commits on main, one per part");
  await pause(3500);
  const log = execFileSync("git", ["log", "--format=%s", "-3", "main"], {
    cwd: app.dir,
    encoding: "utf8",
  });
  expect(log.trim()).toBe(
    "Show who's signed in (app#3)\nSign in with a session (app#2)\nAdd a sessions table (app#1)",
  );

  const video = page.video();
  await context.close();
  if (!video) throw new Error("the context recorded no video");
  const recorded = await video.path();
  execFileSync("ffmpeg", [
    "-loglevel",
    "error",
    "-y",
    "-i",
    recorded,
    "-c:v",
    "libx264",
    "-pix_fmt",
    "yuv420p",
    "-crf",
    "28",
    "-movflags",
    "+faststart",
    join(out, "stacked-items.mp4"),
  ]);
  // A shared palette keeps the GIF a few MB; most frames barely change.
  execFileSync("ffmpeg", [
    "-loglevel",
    "error",
    "-y",
    "-i",
    recorded,
    "-vf",
    "fps=12,scale=960:-1:flags=lanczos,split[a][b];" +
      "[a]palettegen=max_colors=128:stats_mode=diff[p];" +
      "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
    join(out, "stacked-items.gif"),
  ]);
});
