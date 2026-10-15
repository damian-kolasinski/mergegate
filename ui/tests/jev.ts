import { type ChildProcess, spawn } from "node:child_process";
import { createInterface } from "node:readline";

/** What the fake answers for a path: Jev's 0 to 9 score, and a 0 to 1 look. */
export type Answers = {
  scores?: Record<string, number>;
  looks?: Record<string, number>;
};

/** testkit's fake Jev, run as its own process with fixed answers. */
export class FakeJev {
  private constructor(
    private readonly process: ChildProcess,
    /** Points a hub at this fake, with its key set. */
    readonly env: Record<string, string>,
  ) {}

  static async start(answers: Answers): Promise<FakeJev> {
    const bin = process.env.FAKE_JEV_BIN;
    if (!bin)
      throw new Error("FAKE_JEV_BIN is unset: run through playwright test");
    const child = spawn(bin, [JSON.stringify(answers)], {
      stdio: ["pipe", "pipe", "inherit"],
    });
    const lines = createInterface({ input: child.stdout });
    const line = await new Promise<string>((resolve, reject) => {
      lines.once("line", resolve);
      child.once("exit", (code) =>
        reject(new Error(`fake-jev exited with ${code}`)),
      );
    });
    lines.close();
    const ready = JSON.parse(line) as { env: Record<string, string> };
    return new FakeJev(child, ready.env);
  }

  stop() {
    // Closing stdin is how it's told to stop.
    this.process.stdin?.end();
  }
}
