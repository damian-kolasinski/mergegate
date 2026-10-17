import { type ChildProcess, spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { createInterface } from "node:readline";
import { git } from "./hub";

/**
 * testkit's fake GitHub, run as its own process: the REST API, git over
 * HTTP, a `gh` that hands out its token, and plain pages for pull requests.
 */
export class FakeGithub {
  private constructor(
    private readonly process: ChildProcess,
    /** Points a hub, and the `gh` it runs, at this fake. */
    readonly env: Record<string, string>,
    private readonly repos: string,
    readonly web: string,
  ) {}

  static async start(): Promise<FakeGithub> {
    const bin = process.env.FAKE_GITHUB_BIN;
    if (!bin)
      throw new Error("FAKE_GITHUB_BIN is unset: run through playwright test");
    const child = spawn(bin, [], { stdio: ["pipe", "pipe", "inherit"] });
    const lines = createInterface({ input: child.stdout });
    const line = await new Promise<string>((resolve, reject) => {
      lines.once("line", resolve);
      child.once("exit", (code) =>
        reject(new Error(`fake-github exited with ${code}`)),
      );
    });
    lines.close();
    const ready = JSON.parse(line) as {
      env: Record<string, string>;
      repos: string;
      web: string;
    };
    return new FakeGithub(child, ready.env, ready.repos, ready.web);
  }

  /** `owner/repo` on GitHub, holding `dir`'s branches; the hub may push to it. */
  createRepo(repo: string, dir: string) {
    const path = join(this.repos, `${repo}.git`);
    mkdirSync(dirname(path), { recursive: true });
    git(dir, "clone", "--quiet", "--bare", dir, path);
  }

  /**
   * Comments on pull request `pull` of `acme/app` as `login`, which is a
   * bot's when it ends in `[bot]`; with `path` and `line`, on those lines of
   * the branch's head. Its ID on GitHub.
   */
  async comment(
    pull: number,
    comment: {
      login: string;
      body: string;
      kind?: "issue" | "inline" | "review";
      path?: string;
      line?: number;
    },
  ): Promise<number> {
    const response = await fetch(`${this.web}/_fake/comments`, {
      method: "POST",
      body: JSON.stringify({
        repo: "acme/app",
        pull,
        kind: comment.kind ?? (comment.line ? "inline" : "issue"),
        bot: comment.login.endsWith("[bot]"),
        ...comment,
      }),
    });
    if (!response.ok)
      throw new Error(`fake-github answered ${response.status}`);
    return ((await response.json()) as { id: number }).id;
  }

  /**
   * Takes the next `count` numbers of `repo`'s pull requests, as ones opened
   * without the hub would, so its items' pull requests and IDs part ways.
   */
  async openElsewhere(repo: string, count: number) {
    const response = await fetch(`${this.web}/_fake/pulls`, {
      method: "POST",
      body: JSON.stringify({ repo, count }),
    });
    if (!response.ok)
      throw new Error(`fake-github answered ${response.status}`);
  }

  /**
   * Merges or closes pull request `pull` of `acme/app` on GitHub, as someone
   * there would, without the hub.
   */
  async onGithub(pull: number, action: "merge" | "close") {
    const response = await fetch(`${this.web}/_fake/pulls/${pull}/${action}`, {
      method: "POST",
      body: JSON.stringify({ repo: "acme/app" }),
    });
    if (!response.ok)
      throw new Error(`fake-github answered ${response.status}`);
  }

  stop() {
    // Closing stdin is how it's told to stop.
    this.process.stdin?.end();
  }
}
