import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

export interface E2eEnv {
  baseURL: string;
  pid: number | null;
}

const runnerDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../runner",
);

const envFilePath = path.join(runnerDir, ".e2e-env.json");

export function loadE2eEnv(): E2eEnv {
  if (process.env.E2E_BASE_URL) {
    return {
      baseURL: process.env.E2E_BASE_URL.replace(/\/$/, ""),
      pid: null,
    };
  }

  if (fs.existsSync(envFilePath)) {
    const raw = JSON.parse(fs.readFileSync(envFilePath, "utf8")) as E2eEnv;
    return {
      baseURL: raw.baseURL.replace(/\/$/, ""),
      pid: raw.pid ?? null,
    };
  }

  throw new Error(
    "E2E base URL not configured. Set E2E_BASE_URL or run via e2e/runner global setup.",
  );
}

export function loadBaseUrl(): string {
  return loadE2eEnv().baseURL;
}
