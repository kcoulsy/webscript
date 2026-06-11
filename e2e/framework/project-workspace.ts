import * as fs from "node:fs/promises";
import * as fsSync from "node:fs";
import * as path from "node:path";
import { tmpdir } from "node:os";
import type { Page } from "@playwright/test";
import { expect } from "@playwright/test";
import {
  runWebCheck,
  runWebDbGenerate,
  runWebDbMigrate,
  startWebServer,
  type CommandResult,
} from "./web-cli.js";
import {
  findFreePort,
  stopWebScriptServer,
  waitForHealthy,
} from "./net.js";

export interface ProjectWorkspaceOptions {
  repoRoot: string;
  sourceDir: string;
}

export class ProjectWorkspace {
  readonly repoRoot: string;
  readonly sourceDir: string;
  readonly root: string;

  private server: ReturnType<typeof startWebServer> | null = null;
  private serverPid: number | null = null;
  private _baseURL: string | null = null;

  private constructor(options: ProjectWorkspaceOptions, root: string) {
    this.repoRoot = options.repoRoot;
    this.sourceDir = options.sourceDir;
    this.root = root;
  }

  static async create(options: ProjectWorkspaceOptions): Promise<ProjectWorkspace> {
    const root = await fs.mkdtemp(path.join(tmpdir(), "webscript-e2e-"));
    return new ProjectWorkspace(options, root);
  }

  get baseURL(): string {
    if (!this._baseURL) {
      throw new Error("Server not started. Call startServer() first.");
    }
    return this._baseURL;
  }

  async materialize(): Promise<void> {
    await copyDirectory(this.sourceDir, this.root);
  }

  async write(relativePath: string, content: string): Promise<void> {
    const target = path.join(this.root, relativePath);
    await fs.mkdir(path.dirname(target), { recursive: true });
    await fs.writeFile(target, content, "utf8");
  }

  async read(relativePath: string): Promise<string> {
    return fs.readFile(path.join(this.root, relativePath), "utf8");
  }

  async check(): Promise<CommandResult> {
    return runWebCheck(this.repoRoot, this.root);
  }

  async assertCheckOk(): Promise<CommandResult> {
    const result = await this.check();
    expect(
      result.ok,
      `Expected web check to pass.\n${result.output}`,
    ).toBe(true);
    return result;
  }

  async assertCheckFails(matcher?: RegExp): Promise<CommandResult> {
    const result = await this.check();
    expect(
      result.ok,
      "Expected web check to fail but it passed.",
    ).toBe(false);
    if (matcher) {
      expect(result.output).toMatch(matcher);
    }
    return result;
  }

  async setupDatabase(migrationName = "init"): Promise<void> {
    const gen = await runWebDbGenerate(this.repoRoot, this.root, migrationName);
    expect(gen.ok, `web db:generate failed.\n${gen.output}`).toBe(true);
    const mig = await runWebDbMigrate(this.repoRoot, this.root);
    expect(mig.ok, `web db:migrate failed.\n${mig.output}`).toBe(true);
  }

  async startServer(workerSlot = 0): Promise<string> {
    if (this._baseURL) {
      return this._baseURL;
    }

    const port = await findFreePort(34567 + workerSlot * 47);
    const child = startWebServer(this.repoRoot, this.root, port);
    this.server = child;
    this.serverPid = child.pid ?? null;
    this._baseURL = `http://127.0.0.1:${port}`;

    child.stdout?.on("data", (chunk: Buffer) => {
      process.stdout.write(`[web:${port}] ${chunk.toString()}`);
    });
    child.stderr?.on("data", (chunk: Buffer) => {
      process.stderr.write(`[web:${port}] ${chunk.toString()}`);
    });

    await waitForHealthy(this._baseURL);
    return this._baseURL;
  }

  async goto(page: Page, urlPath: string): Promise<void> {
    await page.goto(new URL(urlPath, this.baseURL).toString());
  }

  async dispose(): Promise<void> {
    stopWebScriptServer(this.serverPid);
    this.server = null;
    this.serverPid = null;
    this._baseURL = null;

    try {
      await fs.rm(this.root, { recursive: true, force: true });
    } catch {
      // Best effort cleanup.
    }
  }
}

export function findRepoRoot(startDir: string): string {
  let current = path.resolve(startDir);
  while (true) {
    const cargo = path.join(current, "Cargo.toml");
    if (fsSync.existsSync(cargo)) {
      const contents = fsSync.readFileSync(cargo, "utf8");
      if (contents.includes('name = "webscript"')) {
        return current;
      }
    }
    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }
  throw new Error(`Could not find WebScript repo root from ${startDir}`);
}

export function colocatedProjectDir(specFile: string): string {
  return path.join(path.dirname(specFile), "project");
}

async function copyDirectory(source: string, destination: string): Promise<void> {
  await fs.mkdir(destination, { recursive: true });
  const entries = await fs.readdir(source, { withFileTypes: true });

  for (const entry of entries) {
    const from = path.join(source, entry.name);
    const to = path.join(destination, entry.name);
    if (entry.isDirectory()) {
      await copyDirectory(from, to);
    } else if (entry.isFile()) {
      await fs.copyFile(from, to);
    }
  }
}
