import { defineConfig } from "@playwright/test";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const runnerDir = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  testDir: path.resolve(runnerDir, "../tests"),
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: process.env.CI ? 2 : undefined,
  reporter: [["list"], ["html", { open: "never" }]],
  globalSetup: path.join(runnerDir, "global-setup.ts"),
  globalTeardown: path.join(runnerDir, "global-teardown.ts"),
  use: {
    trace: "on-first-retry",
    headless: true,
  },
});
