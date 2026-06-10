import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { runCargoBuild } from "../framework/web-cli.js";

const runnerDir = path.dirname(fileURLToPath(import.meta.url));
const envFilePath = path.join(runnerDir, ".e2e-env.json");
const repoRoot = path.resolve(runnerDir, "../..");

export default async function globalSetup(): Promise<void> {
  const build = await runCargoBuild(repoRoot);
  if (!build.ok) {
    throw new Error(`Failed to build web binary:\n${build.output}`);
  }

  if (process.env.E2E_SKIP_SERVER === "1") {
    const baseURL = process.env.E2E_BASE_URL;
    if (!baseURL) {
      throw new Error("E2E_SKIP_SERVER requires E2E_BASE_URL");
    }
    fs.writeFileSync(
      envFilePath,
      JSON.stringify({ baseURL, pid: null }, null, 2),
    );
  }
}
