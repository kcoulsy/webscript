import { expect, test } from "../../../framework/fixture.js";
import { open } from "../../../framework/ws.js";

test.describe("tailwind-like utilities", () => {
  test("serves generated stylesheet and applies utility classes", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/", { hydrate: false });

    await expect(
      page.locator('link[href="/.web/tailwind.css"]'),
    ).toHaveCount(1);

    const response = await page.request.get(
      new URL("/.web/tailwind.css", workspace.baseURL).toString(),
    );
    expect(response.status()).toBe(200);
    await expect(response.text()).resolves.toContain("display: flex");

    const panel = page.getByTestId("tailwind-panel");
    await expect(panel).toBeVisible();
    await expect(panel).toHaveCSS("display", "flex");
    await expect(panel).toHaveCSS("background-color", "rgb(59, 130, 246)");
    await expect(panel).toHaveCSS("color", "rgb(255, 255, 255)");
  });
});
