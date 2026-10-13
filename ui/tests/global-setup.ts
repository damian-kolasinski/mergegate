import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

/**
 * Builds the `mg` under test, with the UI this checkout holds: `MG_REQUIRE_UI`
 * fails the build instead of embedding the stub page. `MG_BIN` skips the
 * build and tests that binary instead.
 */
export default function globalSetup() {
  if (process.env.MG_BIN) return;
  const output = execFileSync(
    process.env.CARGO ?? "cargo",
    ["build", "--quiet", "--bin", "mg", "--message-format=json"],
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
    if (message.target?.name === "mg" && message.executable) {
      process.env.MG_BIN = message.executable;
      return;
    }
  }
  throw new Error("cargo built no mg executable");
}
