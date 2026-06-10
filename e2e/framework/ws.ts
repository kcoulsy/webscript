import type { Page } from "@playwright/test";
import { waitForWebScriptReady } from "./hydration.js";
import { island, type Island } from "./island.js";
import type { ProjectWorkspace } from "./project-workspace.js";
import type { IslandQuery } from "./selectors.js";

export interface OpenOptions {
  /** Wait for `/.web/runtime.js` and island hydration. Default: auto when islands exist. */
  hydrate?: boolean;
}

/**
 * Navigate to a route in a colocated test project.
 */
export async function open(
  page: Page,
  workspace: ProjectWorkspace,
  route: string,
  options: OpenOptions = {},
): Promise<void> {
  await workspace.goto(page, route);
  await page.waitForLoadState("load");

  const shouldHydrate =
    options.hydrate ??
    (await page.locator("[data-ws-island]").count()) > 0;

  if (shouldHydrate) {
    await waitForWebScriptReady(page);
  }
}

/** Locate a hydrated client island by component name and render order. */
export function wsIsland(page: Page, query: IslandQuery): Island {
  return island(page, query);
}

/** Shorthand for `wsIsland(page, { component, index })`. */
export function wsIslandAt(
  page: Page,
  component: string,
  index = 0,
): Island {
  return island(page, { component, index });
}

/** Absolute URL for a route in the active test project. */
export function url(workspace: ProjectWorkspace, route: string): string {
  return new URL(route, workspace.baseURL).toString();
}
