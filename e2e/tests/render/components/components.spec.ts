import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("component rendering", () => {
  test("server component props render", async ({ page, workspace }) => {
    await open(page, workspace, "/preview", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Post preview");
    await expect(page.getByRole("heading", { name: "Launch day" })).toBeVisible();
    await expect(page.getByText("Rank 1")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Draft notes" })).toBeVisible();
    await expect(page.getByText("Rank 2")).toBeVisible();
  });

  test("namespaced nested component renders", async ({ page, workspace }) => {
    await open(page, workspace, "/preview", { hydrate: false });
    await expect(page.getByText("Featured")).toBeVisible();

    await open(page, workspace, "/card", { hydrate: false });
    await expect(page.getByRole("heading", { name: "Nested card" })).toBeVisible();
    await expect(
      page.getByText("Rendered from a namespaced component"),
    ).toBeVisible();
    await expect(page.getByText("Card body text")).toBeVisible();
  });

  test("multiple client islands update independently", async ({ page, workspace }) => {
    await open(page, workspace, "/counter");

    const score = wsIslandAt(page, "Counter", 0);
    const lives = wsIslandAt(page, "Counter", 1);

    await score.expectText("count", "5");
    await lives.expectText("count", "0");

    await score.clickHandler("click", 0).click();
    await score.expectText("count", "6");
    await lives.expectText("count", "0");

    await lives.clickHandler("click", 0).click();
    await lives.expectText("count", "1");
    await score.expectText("count", "6");
  });

  test("scoped styles applied to client component", async ({ page, workspace }) => {
    await open(page, workspace, "/counter", { hydrate: false });
    await expect(page.locator('[data-ws-style="Counter"]').first()).toBeVisible();
  });

  test("layout wraps page content via slot", async ({ page, workspace }) => {
    await open(page, workspace, "/", { hydrate: false });
    await expect(page.getByRole("navigation", { name: "Main" })).toBeVisible();
    await expect(page.locator("main h1")).toHaveText("Components render fixture");
  });
});
