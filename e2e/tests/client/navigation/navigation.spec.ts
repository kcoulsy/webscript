import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("soft navigation", () => {
  test.beforeEach(async ({ page, workspace }) => {
    await open(page, workspace, "/page-a");
  });

  test("outlet swap resets page island state across routes", async ({ page }) => {
    const counterA = wsIslandAt(page, "Counter", 0);
    await counterA.expectText("count", "1");

    await counterA.clickHandler("click", 0).click();
    await counterA.clickHandler("click", 0).click();
    await counterA.clickHandler("click", 0).click();
    await counterA.clickHandler("click", 0).click();
    await counterA.expectText("count", "5");

    await page.getByRole("link", { name: "Page B" }).click();
    await expect(page).toHaveURL(/\/page-b$/);

    const counterB = wsIslandAt(page, "Counter", 0);
    await counterB.expectText("count", "2");

    await page.getByRole("link", { name: "Page A" }).click();
    await expect(page).toHaveURL(/\/page-a$/);

    await counterA.expectText("count", "1");
  });

  test("uses fetch navigation without a full document reload", async ({ page }) => {
    const marker = await page.evaluate(() => {
      (window as Window & { __wsNavMarker?: number }).__wsNavMarker = 1;
      return true;
    });
    expect(marker).toBe(true);

    await page.getByRole("link", { name: "Page B" }).click();
    await expect(page).toHaveURL(/\/page-b$/);

    const preserved = await page.evaluate(
      () => (window as Window & { __wsNavMarker?: number }).__wsNavMarker === 1,
    );
    expect(preserved).toBe(true);
  });
});
