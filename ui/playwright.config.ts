import { defineConfig, devices } from "@playwright/test";

// Every test starts its own hub (see tests/hub.ts), so they run in parallel,
// except the budget tests: they time frames, and a busy machine's other
// workers would be what they measure. They run alone, after the rest.
const budgets = /within budget/;

export default defineConfig({
  testDir: "tests",
  globalSetup: "./tests/global-setup.ts",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",
  use: { trace: "retain-on-failure" },
  projects: [
    {
      name: "chromium",
      grepInvert: budgets,
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "budgets",
      grep: budgets,
      dependencies: ["chromium"],
      workers: 1,
      // A trace snapshots the whole page around every action, in the page:
      // tens of milliseconds on a large diff, timed as if the app's.
      use: { ...devices["Desktop Chrome"], trace: "off" },
    },
  ],
});
