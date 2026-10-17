// The README's screenshots and its loop recording, taken from a hub staged
// with three projects' worth of believable work. Skipped unless
// MG_SHOTS_DIR names where to write them:
//
//   MG_SHOTS_DIR=$PWD/docs/images MG_SHOTS_PACKS=<dir of pack archives> \
//     pnpm --dir ui e2e readme-shots --project chromium
//
// MG_SHOTS_PACKS is optional: without it the diffs show unhighlighted. The
// recording becomes a GIF through ffmpeg, which has to be on PATH.

import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, git, Hub, test } from "./hub";

const out = process.env.MG_SHOTS_DIR ?? "";
const packs = process.env.MG_SHOTS_PACKS;

test.skip(!out, "MG_SHOTS_DIR is unset");
test.describe.configure({ mode: "serial" });
// Sharp on retina screens, which show the README's 900px images at 2x.
test.use({ deviceScaleFactor: 2 });

const ui = { "x-mergegate-client": "ui", "content-type": "application/json" };

// --- app: a TypeScript API client, landing through GitHub -------------------

const tokenStoreBefore = `import { api } from "../http/api";

export type Tokens = { access: string; refresh: string };

export class TokenStore {
  private tokens: Tokens | null = null;

  get access(): string | null {
    return this.tokens?.access ?? null;
  }

  store(tokens: Tokens): void {
    this.tokens = tokens;
  }

  clear(): void {
    this.tokens = null;
  }
}
`;

const tokenStoreR1 = `import { api } from "../http/api";

export type Tokens = { access: string; refresh: string };

export class TokenStore {
  private tokens: Tokens | null = null;
  private refreshing: Promise<Tokens> | null = null;
  private loggingOut = false;

  get access(): string | null {
    return this.tokens?.access ?? null;
  }

  store(tokens: Tokens): void {
    this.tokens = tokens;
  }

  /** Trades the refresh token for a new pair; concurrent calls share one. */
  async refresh(): Promise<Tokens> {
    if (!this.tokens) throw new SignedOutError();
    this.refreshing ??= api
      .post<Tokens>("/auth/refresh", { token: this.tokens.refresh })
      .finally(() => (this.refreshing = null));
    const tokens = await this.refreshing;
    if (this.loggingOut) throw new SignedOutError();
    this.store(tokens);
    return tokens;
  }

  async logout(): Promise<void> {
    this.loggingOut = true;
    await api.post("/auth/logout");
    this.clear();
    this.loggingOut = false;
  }

  clear(): void {
    this.tokens = null;
  }
}

export class SignedOutError extends Error {}
`;

/** r2: a logout bumps a generation, so a refresh that outlives it can't store. */
const tokenStoreR2 = tokenStoreR1
  .replace("  private loggingOut = false;\n", "  private generation = 0;\n")
  .replace(
    "    const tokens = await this.refreshing;\n    if (this.loggingOut) throw new SignedOutError();\n",
    "    const generation = this.generation;\n    const tokens = await this.refreshing;\n    // A logout while the request was out wins, even once it has finished.\n    if (generation !== this.generation) throw new SignedOutError();\n",
  )
  .replace(
    '    this.loggingOut = true;\n    await api.post("/auth/logout");\n    this.clear();\n    this.loggingOut = false;\n',
    '    this.generation++;\n    await api.post("/auth/logout");\n    this.clear();\n',
  );

const clientBefore = `import type { TokenStore } from "../auth/tokenStore";

export class Client {
  constructor(
    private readonly baseUrl: string,
    private readonly tokens: TokenStore,
  ) {}

  async send(path: string, init: RequestInit = {}): Promise<Response> {
    return fetch(this.baseUrl + path, this.authorize(init));
  }

  private authorize(init: RequestInit): RequestInit {
    const headers = new Headers(init.headers);
    const access = this.tokens.access;
    if (access) headers.set("Authorization", \`Bearer \${access}\`);
    return { ...init, headers };
  }
}
`;

const clientR1 = clientBefore.replace(
  "    return fetch(this.baseUrl + path, this.authorize(init));\n  }\n\n  private",
  `    const response = await fetch(this.baseUrl + path, this.authorize(init));
    if (response.status !== 401) return response;
    // One retry: a second 401 means the refresh token was revoked too.
    await this.tokens.refresh();
    return fetch(this.baseUrl + path, this.authorize(init));
  }

  private`,
);

const sessionBefore = `import { TokenStore } from "./tokenStore";

export const tokens = new TokenStore();

export function signOut(): void {
  tokens.clear();
  location.assign("/sign-in");
}
`;

const sessionR1 = `import { SignedOutError, TokenStore } from "./tokenStore";

export const tokens = new TokenStore();

export function signOut(): void {
  tokens.clear();
  location.assign("/sign-in");
}

/** Sends the user to sign in when a refresh finds them signed out. */
export function onSignedOut(error: unknown): void {
  if (error instanceof SignedOutError) signOut();
  else throw error;
}
`;

const tokenStoreTest = `import { describe, expect, it } from "vitest";
import { fakeApi } from "./fakeApi";
import { SignedOutError, TokenStore } from "../src/auth/tokenStore";

describe("TokenStore.refresh", () => {
  it("shares one request between concurrent 401s", async () => {
    const api = fakeApi({ refresh: { access: "a2", refresh: "r2" } });
    const store = new TokenStore();
    store.store({ access: "a1", refresh: "r1" });
    await Promise.all([store.refresh(), store.refresh()]);
    expect(api.calls("/auth/refresh")).toBe(1);
    expect(store.access).toBe("a2");
  });

  it("keeps the user signed out when logout races the refresh", async () => {
    const api = fakeApi({ refresh: "slow" });
    const store = new TokenStore();
    store.store({ access: "a1", refresh: "r1" });
    const refresh = store.refresh();
    await store.logout();
    api.finish("/auth/refresh", { access: "a2", refresh: "r2" });
    await expect(refresh).rejects.toBeInstanceOf(SignedOutError);
    expect(store.access).toBeNull();
  });
});
`;

const locale = (lang: string, expired?: string) =>
  `${JSON.stringify(
    {
      "auth.signIn": { en: "Sign in", de: "Anmelden", pl: "Zaloguj się" }[lang],
      "auth.signOut": { en: "Sign out", de: "Abmelden", pl: "Wyloguj się" }[
        lang
      ],
      ...(expired ? { "session.expired": expired } : {}),
    },
    null,
    2,
  )}\n`;

const lockfile = (version: string) =>
  `${JSON.stringify(
    {
      name: "acme-app",
      lockfileVersion: 3,
      packages: {
        "": {
          dependencies: { ky: "^1.7.0" },
          devDependencies: { vitest: "^3.2.0" },
        },
        "node_modules/ky": { version: "1.7.2" },
        "node_modules/vitest": { version },
        "node_modules/@vitest/expect": { version },
        "node_modules/@vitest/runner": { version },
      },
    },
    null,
    2,
  )}\n`;

const appGuide = `Retries a request once on 401, after trading the refresh token for a new pair. Concurrent 401s share one refresh, and a logout that races it wins, so a signed-out user stays signed out.

Risk: the logout race in \`TokenStore.refresh\`; the second test covers it.

## Files

- \`src/auth/tokenStore.ts\` — the shared refresh and the logout guard, start here
- \`src/http/client.ts\` — the one retry on 401
- \`src/auth/session.ts\` — sends the user to sign in when the refresh fails
- \`test/tokenStore.test.ts\` — covers the shared refresh and the race
`;

const appRules = `[[rule]]
id = "R1"
tier = "trusted"
path = "**/package-lock.json"
reason = "lockfile; npm writes it"

[[rule]]
id = "R2"
tier = "trusted"
path = "locales/*.json"
reason = "translations; the i18n check runs in CI"
`;

const appSeed = {
  "src/auth/tokenStore.ts": tokenStoreBefore,
  "src/http/client.ts": clientBefore,
  "src/auth/session.ts": sessionBefore,
  "src/routes/login.ts": `import { Router } from "express";

export const login = Router();

login.post("/login", async (req, res) => {
  const user = await authenticate(req.body.email, req.body.password);
  res.json(user ? await issueTokens(user) : { error: "invalid" });
});
`,
  "package-lock.json": lockfile("3.2.0"),
  "locales/en.json": locale("en"),
  "locales/de.json": locale("de"),
  "locales/pl.json": locale("pl"),
};

const appR1 = {
  "src/auth/tokenStore.ts": tokenStoreR1,
  "src/http/client.ts": clientR1,
  "src/auth/session.ts": sessionR1,
  "test/tokenStore.test.ts": tokenStoreTest,
  "package-lock.json": lockfile("3.2.4"),
  "locales/en.json": locale("en", "Your session expired. Signing you back in…"),
  "locales/de.json": locale(
    "de",
    "Sitzung abgelaufen. Du wirst neu angemeldet…",
  ),
  "locales/pl.json": locale("pl", "Sesja wygasła. Logujemy ponownie…"),
};

// --- web: a React front end, landing locally ---------------------------------

const settingsBefore = `export function SettingsScreen() {
  return (
    <main className="settings">
      <h1>Settings</h1>
      <ProfileForm />
      <NotificationToggles />
      <DangerZone />
    </main>
  );
}
`;

const settingsR1 = `import { Section } from "../ui/Section";

export function SettingsScreen() {
  return (
    <main className="settings">
      <h1>Settings</h1>
      <Section title="Profile" description="How others see you">
        <ProfileForm />
      </Section>
      <Section title="Notifications" description="What we email you about">
        <NotificationToggles />
      </Section>
      <Section title="Danger zone" tone="critical">
        <DangerZone />
      </Section>
    </main>
  );
}
`;

// --- helpers -----------------------------------------------------------------

/** A repo registered with the hub, on `backend`, its `main` seeded. */
function project(
  hub: Hub,
  name: string,
  seed: Record<string, string>,
  github?: { createRepo(repo: string, dir: string): void },
): string {
  const dir = join(hub.root, name);
  mkdirSync(dir);
  git(dir, "init", "--quiet", "--initial-branch=main");
  write(dir, seed);
  git(dir, "commit", "--quiet", "-m", "initial");
  if (github) {
    git(dir, "remote", "add", "origin", `git@github.com:acme/${name}.git`);
    github.createRepo(`acme/${name}`, dir);
    hub.mg(
      ["project", "add", "--backend", "github", "--languages", "none"],
      dir,
    );
  } else {
    hub.mg(["project", "add", "--languages", "none"], dir);
  }
  hub.mg(["init"], dir);
  return dir;
}

function write(dir: string, files: Record<string, string>) {
  for (const [path, contents] of Object.entries(files)) {
    const file = join(dir, path);
    mkdirSync(join(file, ".."), { recursive: true });
    writeFileSync(file, contents);
  }
  git(dir, "add", "--all");
}

/** Commits `files` on a new `branch` off `main` and submits it with `flags`. */
function submit(
  hub: Hub,
  dir: string,
  branch: string,
  title: string,
  files: Record<string, string>,
  flags: string[] = [],
) {
  git(dir, "switch", "--quiet", "-c", branch, "main");
  write(dir, files);
  git(dir, "commit", "--quiet", "-m", title);
  hub.mg(["submit", ...flags], dir);
}

/** Commits `files` on `dir`'s branch and submits the next revision. */
function revise(hub: Hub, dir: string, files: Record<string, string>) {
  write(dir, files);
  git(dir, "commit", "--quiet", "-m", "Address review");
  hub.mg(["submit"], dir);
}

/** The 1-based line of `file` holding `text`. */
function lineOf(file: string, text: string): number {
  const index = file.split("\n").findIndex((line) => line.includes(text));
  if (index < 0) throw new Error(`no line holds ${text}`);
  return index + 1;
}

/** Sends a review of `item`'s `revision` with one comment on `path:line`. */
async function review(
  hub: Hub,
  item: string,
  verdict: "comment" | "changes_requested",
  summary: string,
  comment?: { path: string; line: number; body: string },
) {
  const route = `${hub.url}api/items/${encodeURIComponent(item)}`;
  if (comment) {
    const draft = await fetch(`${route}/drafts`, {
      method: "POST",
      headers: ui,
      body: JSON.stringify({
        revision: 1,
        path: comment.path,
        side: "new",
        start_line: comment.line,
        line: comment.line,
        body: comment.body,
      }),
    });
    expect(draft.ok).toBe(true);
  }
  const sent = await fetch(`${route}/reviews`, {
    method: "POST",
    headers: ui,
    body: JSON.stringify({ revision: 1, verdict, summary }),
  });
  expect(sent.ok).toBe(true);
}

async function approve(hub: Hub, item: string) {
  const merged = await fetch(
    `${hub.url}api/items/${encodeURIComponent(item)}/approve`,
    { method: "POST", headers: ui, body: JSON.stringify({ revision: 1 }) },
  );
  expect(merged.ok).toBe(true);
}

/** The IDs of the comments in `item`'s unread events, oldest first. */
function commentIds(hub: Hub, item: string): string[] {
  const feedback = hub.mg(["feedback", item]) as {
    events: { comments?: { id: string }[] }[];
  };
  return feedback.events.flatMap((e) => (e.comments ?? []).map((c) => c.id));
}

/**
 * Spreads the inbox's ages out, as a real queue's are: every item was
 * submitted seconds ago, so each shows `ago` minutes instead.
 */
async function age(page: Page, ago: Record<string, number>) {
  // In the page rather than page.route, which would hold up the event stream.
  await page.addInitScript((ago) => {
    const fetch = window.fetch.bind(window);
    window.fetch = async (input, init) => {
      const response = await fetch(input, init);
      if (!String(input).endsWith("/api/inbox")) return response;
      const inbox = (await response.json()) as Record<
        string,
        { id: string; updated_at: string }[]
      >;
      for (const group of Object.values(inbox)) {
        if (!Array.isArray(group)) continue;
        for (const item of group) {
          const minutes = ago[item.id];
          if (minutes !== undefined) {
            item.updated_at = new Date(
              Date.now() - minutes * 60_000,
            ).toISOString();
          }
        }
      }
      return Response.json(inbox);
    };
  }, ago);
}

/** A screenshot of a sign-in toast, as an agent would attach for its demo. */
async function demoImage(page: Page, path: string) {
  await page.setViewportSize({ width: 390, height: 640 });
  await page.setContent(`<body style="margin:0;font:16px system-ui;background:#f4f6fb">
    <div style="padding:56px 24px 0"><h2 style="margin:0 0 24px">Orders</h2>
    ${["#1042 · Shipped", "#1041 · Delivered", "#1039 · Delivered"]
      .map(
        (o) =>
          `<div style="background:#fff;border-radius:12px;padding:16px;margin-bottom:12px">${o}</div>`,
      )
      .join("")}</div>
    <div style="position:fixed;left:16px;right:16px;bottom:24px;background:#1d2433;color:#fff;border-radius:12px;padding:14px 16px">
      Your session expired. Signing you back in…</div></body>`);
  await page.screenshot({ path });
}

// --- the shots ---------------------------------------------------------------

test("readme inbox and review screen", async ({ page, github }) => {
  const { hub } = github;
  if (packs) hub.mg(["lang", "add", "--from", packs]);
  const demo = join(hub.root, "demo.png");
  await demoImage(page, demo);

  const app = project(hub, "app", appSeed, github.github);
  await hub.rules("app", appRules);
  const guide = join(hub.root, "guide.md");
  writeFileSync(guide, appGuide);
  submit(
    hub,
    app,
    "token-refresh",
    "Retry once on 401 with a refreshed token",
    appR1,
    [
      "--guide",
      guide,
      "--attach",
      demo,
      "--priority",
      "p0",
      "--note",
      "blocks the auth refactor · 3 agents waiting",
    ],
  );
  submit(hub, app, "rate-limit", "Rate-limit the login endpoint", {
    "src/auth/rateLimit.ts": `const attempts = new Map<string, number[]>();

/** True once \`key\` has tried more than \`max\` times in \`windowMs\`. */
export function limited(key: string, max = 5, windowMs = 60_000): boolean {
  const now = Date.now();
  const recent = (attempts.get(key) ?? []).filter((t) => now - t < windowMs);
  attempts.set(key, [...recent, now]);
  return recent.length >= max;
}
`,
  });
  submit(hub, app, "redis-sessions", "Move sessions to Redis", {
    "src/auth/session.ts": sessionBefore.replace(
      'import { TokenStore } from "./tokenStore";',
      'import { RedisSessions } from "./redisSessions";\nimport { TokenStore } from "./tokenStore";',
    ),
  });

  // A review bot comments on the pull request, and the agent turns it down.
  await github.github.comment(1, {
    login: "chatgpt-codex-connector[bot]",
    body: "Consider retrying the refresh with exponential backoff: a transient 5xx here signs the user out.",
    path: "src/auth/tokenStore.ts",
    line: lineOf(tokenStoreR1, '.post<Tokens>("/auth/refresh"'),
  });
  hub.mg(["sync", "app#1"]);
  const [bot] = commentIds(hub, "app#1");
  hub.mg([
    "reply",
    bot ?? "",
    "--status",
    "rejected",
    "--body",
    "One attempt is the contract: the client already retries the request once, and a backoff here would hold every queued 401 behind it.",
  ]);

  const web = project(hub, "web", {
    "src/settings/SettingsScreen.tsx": settingsBefore,
    "src/theme/tokens.css": ":root {\n  --bg: #fff;\n  --text: #111;\n}\n",
    "src/pricing/Pricing.tsx":
      'export const headline = "Simple pricing for teams";\n',
  });
  submit(
    hub,
    web,
    "settings",
    "Redesign the settings screen",
    {
      "src/settings/SettingsScreen.tsx": settingsR1,
      "src/ui/Section.tsx": `export function Section(props: {
  title: string;
  description?: string;
  tone?: "critical";
  children: JSX.Element;
}) {
  return (
    <section class="section" data-tone={props.tone}>
      <h2>{props.title}</h2>
      {props.description && <p>{props.description}</p>}
      {props.children}
    </section>
  );
}
`,
    },
    ["--priority", "p1", "--note", "design review on Friday"],
  );
  await review(hub, "web#1", "changes_requested", "Close to the mock.", {
    path: "src/settings/SettingsScreen.tsx",
    line: lineOf(settingsR1, 'title="Notifications"'),
    body: "The mock groups this under Account with Profile.",
  });
  const [question] = commentIds(hub, "web#1");
  hub.mg([
    "reply",
    question ?? "",
    "--status",
    "question",
    "--body",
    "Should Danger zone move under Account too, or stay last on its own?",
  ]);
  submit(hub, web, "dark-mode", "Add dark mode color tokens", {
    "src/theme/tokens.css":
      ":root {\n  --bg: #fff;\n  --text: #111;\n}\n\n@media (prefers-color-scheme: dark) {\n  :root {\n    --bg: #0f1115;\n    --text: #e8eaf0;\n  }\n}\n",
  });
  await review(
    hub,
    "web#2",
    "changes_requested",
    "Contrast on --text-muted fails AA in dark; check it against --bg.",
  );
  submit(hub, web, "pricing-copy", "Fix the pricing page copy", {
    "src/pricing/Pricing.tsx":
      'export const headline = "Simple pricing for teams of any size";\n',
  });
  await approve(hub, "web#3");

  const ios = project(hub, "ios", {
    "App/Localizable.xcstrings": `{\n  "sourceLanguage" : "en",\n  "strings" : {}\n}\n`,
    "App/InfoPlist.xcstrings": `{\n  "sourceLanguage" : "en",\n  "strings" : {}\n}\n`,
  });
  await hub.rules(
    "ios",
    `[[rule]]\nid = "R1"\ntier = "trusted"\npath = "**/*.xcstrings"\nreason = "string catalogs; Xcode writes them"\n`,
  );
  submit(
    hub,
    ios,
    "polish",
    "Add Polish translations",
    {
      "App/Localizable.xcstrings": `{\n  "sourceLanguage" : "en",\n  "strings" : {\n    "Sign in" : { "localizations" : { "pl" : { "stringUnit" : { "value" : "Zaloguj się" } } } }\n  }\n}\n`,
      "App/InfoPlist.xcstrings": `{\n  "sourceLanguage" : "en",\n  "strings" : {\n    "CFBundleDisplayName" : { "localizations" : { "pl" : { "stringUnit" : { "value" : "Acme" } } } }\n  }\n}\n`,
      "App/Settings/Settings.xcstrings": `{\n  "sourceLanguage" : "en",\n  "strings" : {}\n}\n`,
      "App/Locale+Plural.swift": `import Foundation

extension Locale {
  /// Polish has three plural forms; "few" covers 2–4, but not 12–14.
  func pluralCategory(for count: Int) -> PluralCategory {
    guard language.languageCode == "pl" else { return count == 1 ? .one : .other }
    if count == 1 { return .one }
    let (tens, ones) = (count % 100 / 10, count % 10)
    return (2...4).contains(ones) && tens != 1 ? .few : .many
  }
}
`,
    },
    ["--priority", "p2"],
  );

  await page.setViewportSize({ width: 1440, height: 600 });
  await age(page, {
    "app#1": 42,
    "web#1": 18,
    "ios#1": 130,
    "app#2": 210,
    "app#3": 95,
    "web#2": 26,
    "web#3": 64,
  });
  await page.goto(hub.url);
  await expect(page.getByText("Retry once on 401")).toBeVisible();
  await expect(page.getByText("conflicts with").first()).toBeVisible();
  await page.getByText(/^Closed today/).click();
  await expect(page.getByText("Reconnecting to hub")).toHaveCount(0);
  await page.screenshot({ path: join(out, "inbox.png") });

  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(`${hub.url}#/items/app%231`);
  const screen = page.getByRole("region", { name: "app#1" });
  await expect(screen.getByText("chatgpt-codex-connector[bot]")).toBeVisible();
  await expect(
    screen.getByRole("button", { name: "Accept rejection" }),
  ).toBeVisible();
  // Lifts the bot's thread clear of the review bar, stopping on a fold so
  // no line is cut in half under the file's sticky header.
  const fold = screen.getByText("3 unmodified lines").nth(1);
  const header = screen
    .getByText("the shared refresh and the logout guard, start here")
    .last();
  const [foldBox, headerBox] = [
    await fold.boundingBox(),
    await header.boundingBox(),
  ];
  await page.mouse.move(860, 500);
  await page.mouse.wheel(0, (foldBox?.y ?? 0) - (headerBox?.y ?? 0) - 36);
  await page.waitForTimeout(300);
  await page.screenshot({ path: join(out, "review-screen.png") });
});

test("readme loop recording", async ({ browser }, info) => {
  // Paced for a viewer, not for speed.
  test.setTimeout(120_000);
  const hub = new Hub();
  try {
    if (packs) hub.mg(["lang", "add", "--from", packs]);
    const app = project(hub, "app", appSeed);
    await hub.rules("app", appRules);
    const guide = join(hub.root, "guide.md");
    writeFileSync(guide, appGuide);
    submit(
      hub,
      app,
      "token-refresh",
      "Retry once on 401 with a refreshed token",
      appR1,
      [
        "--guide",
        guide,
        "--priority",
        "p0",
        "--note",
        "blocks the auth refactor · 3 agents waiting",
      ],
    );
    const web = project(hub, "web", {
      "src/settings/SettingsScreen.tsx": settingsBefore,
    });
    submit(
      hub,
      web,
      "settings",
      "Redesign the settings screen",
      {
        "src/settings/SettingsScreen.tsx": settingsR1,
      },
      ["--priority", "p1"],
    );

    const videos = join(info.outputDir, "video");
    const context = await browser.newContext({
      viewport: { width: 1280, height: 800 },
      recordVideo: { dir: videos, size: { width: 1280, height: 800 } },
    });
    const page = await context.newPage();
    const pause = (ms: number) => page.waitForTimeout(ms);
    const caption = (text: string) =>
      page.evaluate((text) => {
        let bar = document.getElementById("readme-caption");
        if (!bar) {
          bar = document.createElement("div");
          bar.id = "readme-caption";
          bar.style.cssText =
            "position:fixed;left:50%;bottom:96px;transform:translateX(-50%);z-index:9999;" +
            "background:#0b1f3a;color:#fff;font:600 18px 'DM Sans Variable',system-ui;" +
            "padding:10px 18px;border-radius:10px;box-shadow:0 6px 24px rgb(0 0 0/.25)";
          document.body.append(bar);
        }
        bar.textContent = text;
      }, text);

    await page.goto(hub.url);
    await expect(page.getByText("Retry once on 401")).toBeVisible();
    await caption("Agents submit their work with mg submit");
    await pause(2500);
    await page.keyboard.press("Enter");
    const screen = page.getByRole("region", { name: "app#1" });
    await expect(screen.getByRole("heading")).toBeVisible();
    await caption("Files ordered by importance, low-risk ones folded away");
    await pause(3000);

    // `]` lands on the first hunk's top line, 4; `j` steps to the guard's.
    await caption("You comment on a line…");
    await page.keyboard.press("]");
    for (
      let line = 4;
      line < lineOf(tokenStoreR1, "private loggingOut");
      line++
    ) {
      await pause(250);
      await page.keyboard.press("j");
    }
    await pause(400);
    await page.keyboard.press("c");
    const editor = page.getByRole("textbox", {
      name: `Comment on src/auth/tokenStore.ts line ${lineOf(tokenStoreR1, "private loggingOut")}`,
    });
    await expect(editor).toBeFocused();
    await page.keyboard.type(
      "logout() sets this back to false when it finishes, so a refresh that resolves after that still stores tokens.",
      { delay: 18 },
    );
    await page.getByRole("button", { name: "Save draft" }).click();
    await pause(600);
    await caption("v marks a file viewed and moves on");
    for (let file = 0; file < 4; file++) {
      await page.keyboard.press("v");
      await pause(700);
    }
    await page.getByRole("textbox", { name: "Review summary" }).click();
    await page.keyboard.type("One race left; the rest looks right.", {
      delay: 18,
    });
    await screen.getByRole("button", { name: "Request changes" }).click();
    await caption("…and it goes straight back to the agent");
    await pause(2500);

    // The agent answers and submits the fix.
    const [comment] = commentIds(hub, "app#1");
    hub.mg(["reply", comment ?? "", "--status", "addressed"]);
    revise(hub, app, { "src/auth/tokenStore.ts": tokenStoreR2 });
    await caption("The agent fixes it and submits r2");
    await expect(page.getByText(/r2 submitted/)).toBeVisible();
    await pause(1500);
    await page.getByRole("button", { name: "Show r2" }).click();
    await page.getByLabel("Compare").selectOption("1");
    await caption("You review only what changed since r1");
    await pause(3500);
    // The other files kept their viewed marks: only this one changed.
    await page.keyboard.press("v");
    await expect(page.getByText("4 files not reviewed yet")).toHaveCount(0);
    await pause(600);

    await page.keyboard.press("a");
    await caption("Approve & merge: one squash commit, authored by you");
    await pause(1500);
    await page.keyboard.press("Enter");
    await expect(page.getByText(/Merged into main/)).toBeVisible();
    await pause(2500);

    const video = page.video();
    await context.close();
    if (!video) throw new Error("the context recorded no video");
    // A shared palette keeps the GIF a few MB; most frames barely change.
    execFileSync("ffmpeg", [
      "-loglevel",
      "error",
      "-y",
      "-i",
      await video.path(),
      "-vf",
      "fps=12,scale=960:-1:flags=lanczos,split[a][b];" +
        "[a]palettegen=max_colors=128:stats_mode=diff[p];" +
        "[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle",
      join(out, "review-loop.gif"),
    ]);
  } finally {
    hub.stop();
  }
});
