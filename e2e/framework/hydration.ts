import type { Page } from "@playwright/test";

export async function waitForWebScriptReady(page: Page): Promise<void> {
  await page.waitForFunction(
    () =>
      typeof (window as Window & { WebScript?: { signal?: unknown } }).WebScript
        ?.signal === "function",
  );

  const islands = page.locator("[data-ws-island]");
  const count = await islands.count();
  if (count > 0) {
    await islands.first().waitFor({ state: "attached" });
  }
}
