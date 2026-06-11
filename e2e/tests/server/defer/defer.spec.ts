import { expect, test } from "../../../framework/fixture.js";
import { open } from "../../../framework/ws.js";

test.describe("@defer streaming", () => {
  test("streams placeholder then deferred content", async ({ page, workspace }) => {
    await open(page, workspace, "/defer-demo", { hydrate: false });
    await expect(page.locator("h1")).toHaveText("Defer streaming demo");
    await expect(page.locator(".shell-note")).toBeVisible();
    await expect(page.locator(".defer-result")).toBeVisible({ timeout: 5000 });
    await expect(page.locator(".defer-result")).toContainText("Loaded after 300ms delay.");
    await expect(page.locator(".defer-placeholder")).toHaveCount(0);
  });
});
