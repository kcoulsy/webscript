import { expect, test } from "../../../framework/fixture.js";
import { open } from "../../../framework/ws.js";

test.describe("@load server async", () => {
  test("fetch with @schema populates template", async ({ page, workspace }) => {
    await open(page, workspace, "/fetch-demo", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Fetch demo");
    await expect(page.getByText(/^Title:/)).toContainText("delectus");
    await expect(page.getByText(/^Error:\s*$/)).toBeVisible();
  });

  test("sleep, spawn, and timeout in @load", async ({ page, workspace }) => {
    await open(page, workspace, "/async-demo", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Async demo");
    await expect(page.getByText(/Steps from/)).toContainText("5");
    await expect(page.getByText(/timeout fired after 5 steps/)).toBeVisible();
    await expect(page.getByText("Timed out: true")).toBeVisible();
  });
});
