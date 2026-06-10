import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("@client signal<bool> and reactive @if", () => {
  test.beforeEach(async ({ page, workspace }) => {
    await open(page, workspace, "/");
  });

  test("bool signal toggles data-ws-branch visibility", async ({ page }) => {
    const island = wsIslandAt(page, "Details", 0);
    const panel = island.branch("open", "then");

    await expect(panel).toBeHidden();
    await island.clickHandler("click", 0).click();
    await expect(panel).toBeVisible();

    await island.clickHandler("click", 0).click();
    await expect(panel).toBeHidden();
  });
});
