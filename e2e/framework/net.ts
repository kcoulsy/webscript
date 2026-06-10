import { spawn } from "node:child_process";
import * as net from "node:net";

export async function findFreePort(start = 34567): Promise<number> {
  for (let port = start; port < start + 200; port++) {
    const available = await isPortAvailable(port);
    if (available) {
      return port;
    }
  }
  throw new Error(`No free port found starting at ${start}`);
}

function isPortAvailable(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.once("error", () => resolve(false));
    server.once("listening", () => {
      server.close(() => resolve(true));
    });
    server.listen(port, "127.0.0.1");
  });
}

export async function waitForHealthy(
  baseURL: string,
  timeoutMs = 60_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${baseURL}/`);
      if (response.status > 0 && response.status < 500) {
        return;
      }
    } catch {
      // Server still starting.
    }
    await delay(250);
  }

  throw new Error(`Server not healthy at ${baseURL} within ${timeoutMs}ms`);
}

export function stopWebScriptServer(pid: number | null): void {
  if (!pid) {
    return;
  }

  if (process.platform === "win32") {
    spawn("taskkill", ["/PID", String(pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
    return;
  }

  try {
    process.kill(pid, "SIGTERM");
  } catch {
    // Process may already be gone.
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
