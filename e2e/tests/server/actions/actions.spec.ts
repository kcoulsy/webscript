import { expect, test } from "../../../framework/fixture.js";
import {
  expectRedirect,
  submitAction,
  submitActionRaw,
} from "../../../framework/server-actions.js";
import { open } from "../../../framework/ws.js";

test.describe("@action", () => {
  test("redirect after mutation", async ({ page, workspace }) => {
    await open(page, workspace, "/", { hydrate: false });
    await submitAction(page, workspace, "/", "increment");
    await expect(page).toHaveURL(new RegExp(`${workspace.baseURL}/?$`));
    await expect(page.getByText("Session count: 1")).toBeVisible();
  });

  test("fail() returns 422", async ({ page, workspace }) => {
    const response = await submitActionRaw(
      page,
      workspace,
      "/",
      "rememberName",
      { name: "" },
    );
    expect(response.status()).toBe(422);
    await expect(response.text()).resolves.toContain("Name is required");
  });

  test("form post persists session field", async ({ page, workspace }) => {
    await open(page, workspace, "/", { hydrate: false });
    await page.locator("#home-name").fill("Ada");
    await page.getByRole("button", { name: "Remember name" }).click();
    await page.waitForURL(new RegExp(`${workspace.baseURL}/?$`));
    await expect(page.getByText("Remembered name: Ada")).toBeVisible();
  });

  test("while validation path rejects empty input", async ({
    page,
    workspace,
  }) => {
    const response = await submitActionRaw(
      page,
      workspace,
      "/",
      "validateWithWhile",
      { name: "" },
    );
    expect(response.status()).toBe(422);
    await expect(response.text()).resolves.toContain("Name still required");
  });

  test("while validation path accepts valid input", async ({
    page,
    workspace,
  }) => {
    const response = await submitActionRaw(
      page,
      workspace,
      "/",
      "validateWithWhile",
      { name: "Grace" },
    );
    await expectRedirect(response, "/");

    await open(page, workspace, "/", { hydrate: false });
    await expect(page.getByText("Remembered name: Grace")).toBeVisible();
  });
});
