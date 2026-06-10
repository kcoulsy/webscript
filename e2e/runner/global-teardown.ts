import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const runnerDir = path.dirname(fileURLToPath(import.meta.url));
const envFilePath = path.join(runnerDir, ".e2e-env.json");

export default async function globalTeardown(): Promise<void> {
  if (!fs.existsSync(envFilePath)) {
    return;
  }

  try {
    fs.unlinkSync(envFilePath);
  } catch {
    // Best effort cleanup.
  }
}
