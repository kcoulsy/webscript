import { expect, type Locator, type Page } from "@playwright/test";
import {
  eventHandlerSelector,
  islandSelector,
  signalBranchSelector,
  signalTextSelector,
  signalValueSelector,
  type IslandQuery,
} from "./selectors.js";

export class Island {
  readonly root: Locator;

  constructor(page: Page, query: IslandQuery) {
    this.root = page.locator(islandSelector(query));
  }

  textOf(signal: string): Locator {
    return this.root.locator(signalTextSelector(signal));
  }

  valueOf(signal: string): Locator {
    return this.root.locator(signalValueSelector(signal));
  }

  branch(signal: string, branch: "then" | "else"): Locator {
    return this.root.locator(signalBranchSelector(signal, branch));
  }

  clickHandler(event: string, index: number): Locator {
    return this.root.locator(eventHandlerSelector(event, index));
  }

  async expectText(
    signal: string,
    value: string,
    options: { nth?: number } = {},
  ): Promise<void> {
    const binding =
      options.nth === undefined
        ? this.textOf(signal).first()
        : this.textOf(signal).nth(options.nth);
    await expect(binding).toHaveText(value);
  }

  async expectVisible(): Promise<void> {
    await expect(this.root).toBeVisible();
  }
}

export function island(page: Page, query: IslandQuery): Island {
  return new Island(page, query);
}
