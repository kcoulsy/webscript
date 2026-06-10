import { expect, test } from "../../../framework/fixture.js";
import { open, url } from "../../../framework/ws.js";

test.describe("@page routing and @layout", () => {
  test("typed path param drives @if branch", async ({ page, workspace }) => {
    await open(page, workspace, "/posts/intro", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Post: intro");
    await expect(page.getByText("@do badge: featured")).toBeVisible();
    await expect(
      page.getByText('route param matched "intro"'),
    ).toBeVisible();
  });

  test("alternate param value selects else branch", async ({ page, workspace }) => {
    await open(page, workspace, "/posts/launch", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Post: launch");
    await expect(page.getByText("@do badge: draft")).toBeVisible();
    await expect(page.getByText("Try /posts/intro")).toBeVisible();
  });

  test("@layout wraps page content", async ({ page, workspace }) => {
    await open(page, workspace, "/", { hydrate: false });
    await expect(page.getByRole("navigation", { name: "Main" })).toBeVisible();
  });

  test("unknown route returns 404", async ({ page, workspace }) => {
    const response = await page.goto(url(workspace, "/does-not-exist"));
    expect(response?.status()).toBe(404);
  });
});
