import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test as base } from "@playwright/test";

/**
 * One isolated mergegate install with its own running hub, as testkit's
 * `Env` is for the Rust tests. State comes from real `mg` and git commands.
 */
export class Hub {
  readonly root = realpathSync(mkdtempSync(join(tmpdir(), "mg-ui-")));
  readonly url: string;

  constructor() {
    this.mg(["hub", "start", "--port", "0"]);
    const info = JSON.parse(
      readFileSync(join(this.root, "home", "hub.json"), "utf8"),
    ) as { addr: string };
    this.url = `http://${info.addr}/`;
  }

  /** Runs `mg --json` in `cwd` and returns what it printed. */
  mg(args: string[], cwd = this.root): unknown {
    const stdout = execFileSync(mgBin(), ["--json", ...args], {
      cwd,
      env: { ...isolated(), MERGEGATE_HOME: join(this.root, "home") },
      encoding: "utf8",
    });
    return JSON.parse(stdout);
  }

  /** A repo named `name` with one commit on `main`, registered and pushed to. */
  project(name: string): Project {
    const dir = join(this.root, name);
    mkdirSync(dir);
    git(dir, "init", "--quiet", "--initial-branch=main");
    git(dir, "commit", "--quiet", "--allow-empty", "-m", "initial");
    this.mg(["project", "add"], dir);
    this.mg(["init"], dir);
    return new Project(this, dir);
  }

  stop() {
    try {
      this.mg(["hub", "stop"]);
    } finally {
      rmSync(this.root, { recursive: true, force: true });
    }
  }
}

export class Project {
  constructor(
    private readonly hub: Hub,
    readonly dir: string,
  ) {}

  /** Commits `files` on a new `branch` off `main` and submits it. */
  submit(branch: string, title: string, files: Record<string, string>) {
    git(this.dir, "switch", "--quiet", "-c", branch, "main");
    this.revise(title, files);
  }

  /** Commits `files` on the current branch and submits the new revision. */
  revise(title: string, files: Record<string, string>) {
    for (const [path, contents] of Object.entries(files)) {
      mkdirSync(dirname(join(this.dir, path)), { recursive: true });
      writeFileSync(join(this.dir, path), contents);
    }
    git(this.dir, "add", "--all");
    git(this.dir, "commit", "--quiet", "-m", title);
    this.hub.mg(["submit"], this.dir);
  }
}

export const test = base.extend<{ hub: Hub }>({
  hub: async ({}, use) => {
    const hub = new Hub();
    try {
      await use(hub);
    } finally {
      hub.stop();
    }
  },
});

export { expect } from "@playwright/test";

function mgBin(): string {
  const bin = process.env.MG_BIN;
  if (!bin) throw new Error("MG_BIN is unset: run through playwright test");
  return bin;
}

function git(dir: string, ...args: string[]) {
  execFileSync("git", args, { cwd: dir, env: isolated(), stdio: "ignore" });
}

/** This process's environment without the developer's git or mg settings. */
function isolated(): NodeJS.ProcessEnv {
  const env = Object.fromEntries(
    Object.entries(process.env).filter(
      ([key]) =>
        !key.startsWith("GIT_") && key !== "MG_JSON" && key !== "MG_UI_DIR",
    ),
  );
  return {
    ...env,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_AUTHOR_NAME: "Test",
    GIT_AUTHOR_EMAIL: "test@example.com",
    GIT_COMMITTER_NAME: "Test",
    GIT_COMMITTER_EMAIL: "test@example.com",
  };
}
