import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

/** The binaries the tests run, by name, and the variable naming each. */
const BINS = { mg: "MG_BIN", "fake-github": "FAKE_GITHUB_BIN" } as const;

/**
 * Builds the `mg` under test, with the UI this checkout holds: `MG_REQUIRE_UI`
 * fails the build instead of embedding the stub page. Also builds testkit's
 * `fake-github`. `MG_BIN` or `FAKE_GITHUB_BIN` skips a build and tests that
 * binary instead.
 */
export default function globalSetup() {
  const missing = Object.entries(BINS)
    .filter(([, variable]) => !process.env[variable])
    .map(([bin]) => bin);
  if (!missing.length) return;
  const output = execFileSync(
    process.env.CARGO ?? "cargo",
    [
      "build",
      "--quiet",
      ...missing.flatMap((bin) => ["--bin", bin]),
      "--message-format=json",
    ],
    {
      cwd: resolve(import.meta.dirname, "../.."),
      env: { ...process.env, MG_REQUIRE_UI: "1" },
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
      stdio: ["ignore", "pipe", "inherit"],
    },
  );
  for (const line of output.split("\n")) {
    if (!line.startsWith("{")) continue;
    const message = JSON.parse(line) as {
      reason?: string;
      target?: { name?: string };
      executable?: string | null;
    };
    const name = message.target?.name;
    if (name && name in BINS && message.executable) {
      process.env[BINS[name as keyof typeof BINS]] = message.executable;
    }
  }
  for (const bin of missing) {
    if (!process.env[BINS[bin as keyof typeof BINS]]) {
      throw new Error(`cargo built no ${bin} executable`);
    }
  }
}
