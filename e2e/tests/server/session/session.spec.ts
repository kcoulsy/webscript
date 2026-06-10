import { expect, test } from "../../../framework/fixture.js";
import { open } from "../../../framework/ws.js";

test.describe("session", () => {
  test("issues webscript_session cookie on first request", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/", { hydrate: false });

    const session = (await page.context().cookies()).find(
      (cookie) => cookie.name === "webscript_session",
    );
    expect(session).toBeDefined();
    expect(session?.httpOnly).toBe(true);
  });

  test("session scope persists across requests in one context", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/", { hydrate: false });
    await expect(page.getByText("Session count: 0")).toBeVisible();

    await page.getByRole("button", { name: "Increment session" }).click();
    await page.waitForURL("**/");
    await expect(page.getByText("Session count: 1")).toBeVisible();

    await open(page, workspace, "/", { hydrate: false });
    await expect(page.getByText("Session count: 1")).toBeVisible();
  });
});
