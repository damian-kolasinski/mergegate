import { type ChildProcess, execFileSync, spawn } from "node:child_process";
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
import { test as base, expect } from "@playwright/test";
import type { Rules } from "../api/Rules";
import { FakeGithub } from "./github";

/**
 * One isolated mergegate install with its own running hub, as testkit's
 * `Env` is for the Rust tests. State comes from real `mg` and git commands.
 */
export class Hub {
  readonly root = realpathSync(mkdtempSync(join(tmpdir(), "mg-ui-")));
  readonly url: string;
  /**
   * `HOME` for every `mg` and the hub, so nothing reads the user's own, such
   * as the agents the inbox offers to set up.
   */
  readonly user = join(this.root, "user");

  /** `env` is set for every `mg`, and so for the hub, e.g. a fake GitHub's. */
  constructor(private readonly env: Record<string, string> = {}) {
    mkdirSync(this.user);
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
      env: {
        ...isolated(),
        ...this.env,
        MERGEGATE_HOME: join(this.root, "home"),
        HOME: this.user,
      },
      encoding: "utf8",
    });
    return JSON.parse(stdout);
  }

  /**
   * Starts an agent's `mg wait` on `item`, as one blocked on its feedback
   * is; kill it to end the wait. `stop` kills any still running.
   */
  wait(item: string): ChildProcess {
    const wait = spawn(mgBin(), ["wait", item, "--timeout", "0"], {
      cwd: this.root,
      env: {
        ...isolated(),
        ...this.env,
        MERGEGATE_HOME: join(this.root, "home"),
        HOME: this.user,
      },
      stdio: "ignore",
    });
    this.waits.push(wait);
    return wait;
  }

  private readonly waits: ChildProcess[] = [];

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

  /**
   * As `project`, but landing through pull requests on `acme/{name}`, which
   * `github` serves.
   */
  githubProject(name: string, github: FakeGithub): Project {
    const dir = join(this.root, name);
    mkdirSync(dir);
    git(dir, "init", "--quiet", "--initial-branch=main");
    git(dir, "commit", "--quiet", "--allow-empty", "-m", "initial");
    git(dir, "remote", "add", "origin", `git@github.com:acme/${name}.git`);
    github.createRepo(`acme/${name}`, dir);
    this.mg(["project", "add", "--backend", "github"], dir);
    this.mg(["init"], dir);
    return new Project(this, dir);
  }

  /**
   * Writes `project`'s rules.toml where the hub reads it, and approves it as
   * the operator, as a trusted rule in it waits for them.
   */
  async rules(project: string, text: string) {
    const dir = join(this.root, "home", "projects", project);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "rules.toml"), text);
    const route = `${this.url}api/projects/${project}/rules`;
    const read = (await (await fetch(route)).json()) as Rules;
    if (read.edit === null) return;
    const approved = await fetch(`${route}/edit/approve`, {
      method: "POST",
      headers: { "x-mergegate-client": "ui" },
    });
    expect(approved.status).toBe(200);
  }

  stop() {
    for (const wait of this.waits) wait.kill();
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

  /** Commits `files` on `main`, where later branches start. */
  seed(files: Record<string, string | Buffer>) {
    git(this.dir, "switch", "--quiet", "main");
    this.write(files);
    git(this.dir, "commit", "--quiet", "-m", "seed");
  }

  /**
   * Commits `files` on a new `branch` off `main` and submits it, with the
   * files at the `attach` paths attached.
   */
  submit(
    branch: string,
    title: string,
    files: Record<string, string | Buffer | null>,
    attach: string[] = [],
  ) {
    git(this.dir, "switch", "--quiet", "-c", branch, "main");
    this.revise(title, files, attach);
  }

  /**
   * Commits `files` on a new `branch` off the current one and submits it
   * stacked on `on`, the item that branch is.
   */
  stack(
    branch: string,
    on: string,
    title: string,
    files: Record<string, string | Buffer | null>,
  ) {
    git(this.dir, "switch", "--quiet", "-c", branch);
    this.write(files);
    git(this.dir, "commit", "--quiet", "-m", title);
    this.hub.mg(["submit", "--on", on], this.dir);
  }

  switch(branch: string) {
    git(this.dir, "switch", "--quiet", branch);
  }

  /** Commits `files` on the current branch and submits the new revision. */
  revise(
    title: string,
    files: Record<string, string | Buffer | null>,
    attach: string[] = [],
  ) {
    this.write(files);
    git(this.dir, "commit", "--quiet", "-m", title);
    this.hub.mg(
      ["submit", ...attach.flatMap((file) => ["--attach", file])],
      this.dir,
    );
  }

  /** Writes `files` and stages them; `null` deletes one. */
  private write(files: Record<string, string | Buffer | null>) {
    for (const [path, contents] of Object.entries(files)) {
      const file = join(this.dir, path);
      if (contents === null) {
        rmSync(file);
        continue;
      }
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, contents);
    }
    git(this.dir, "add", "--all");
  }
}

export const test = base.extend<{
  hub: Hub;
  github: { hub: Hub; github: FakeGithub };
}>({
  hub: async ({}, use) => {
    const hub = new Hub();
    try {
      await use(hub);
    } finally {
      hub.stop();
    }
  },
  /** A hub signed in to a fake GitHub, which starts first. */
  github: async ({}, use) => {
    const github = await FakeGithub.start();
    try {
      // Polled often, so comments posted there show up while a test waits.
      const hub = new Hub({ ...github.env, MG_GITHUB_POLL_MS: "200" });
      try {
        await use({ hub, github });
      } finally {
        hub.stop();
      }
    } finally {
      github.stop();
    }
  },
});

export { expect };

function mgBin(): string {
  const bin = process.env.MG_BIN;
  if (!bin) throw new Error("MG_BIN is unset: run through playwright test");
  return bin;
}

export function git(dir: string, ...args: string[]) {
  execFileSync("git", args, { cwd: dir, env: isolated(), stdio: "ignore" });
}

/** This process's environment without the developer's git or mg settings. */
export function isolated(): NodeJS.ProcessEnv {
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
