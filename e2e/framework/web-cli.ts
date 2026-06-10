import { spawn, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";

export interface CommandResult {
  ok: boolean;
  exitCode: number;
  stdout: string;
  stderr: string;
  output: string;
}

export function resolveWebBinary(repoRoot: string): string {
  const binary = process.platform === "win32" ? "web.exe" : "web";
  const release = path.join(repoRoot, "target", "release", binary);
  if (fs.existsSync(release)) {
    return release;
  }
  const debug = path.join(repoRoot, "target", "debug", binary);
  if (fs.existsSync(debug)) {
    return debug;
  }
  throw new Error(
    `WebScript binary not found. Run \`cargo build --bin web\` from ${repoRoot}`,
  );
}

export function runCargoBuild(repoRoot: string): Promise<CommandResult> {
  return runProcess(
    process.platform === "win32" ? "cargo" : "cargo",
    ["build", "--bin", "web"],
    repoRoot,
  );
}

export function runWebCheck(
  repoRoot: string,
  projectRoot: string,
): Promise<CommandResult> {
  const web = resolveWebBinary(repoRoot);
  return runProcess(web, ["check"], projectRoot);
}

export function startWebServer(
  repoRoot: string,
  projectRoot: string,
  port: number,
): ChildProcess {
  const web = resolveWebBinary(repoRoot);
  const args = ["serve", "--host", "127.0.0.1", "--port", String(port)];

  if (process.platform === "win32") {
    return spawn(web, args, {
      cwd: projectRoot,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    });
  }

  return spawn(web, args, {
    cwd: projectRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function runProcess(
  command: string,
  args: string[],
  cwd: string,
): Promise<CommandResult> {
  return new Promise((resolve, reject) => {
    const child =
      process.platform === "win32"
        ? spawn("cmd", ["/c", command, ...args], {
            cwd,
            windowsHide: true,
          })
        : spawn(command, args, { cwd });

    let stdout = "";
    let stderr = "";

    child.stdout?.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr?.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on("error", reject);
    child.on("close", (code) => {
      const exitCode = code ?? 1;
      resolve({
        ok: exitCode === 0,
        exitCode,
        stdout,
        stderr,
        output: `${stdout}\n${stderr}`.trim(),
      });
    });
  });
}
