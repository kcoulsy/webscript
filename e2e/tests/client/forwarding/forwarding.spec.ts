import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("component forwarded attributes", () => {
  test("web check accepts data-ws-bind inside component branches", async ({
    workspace,
  }) => {
    expect((await workspace.check()).ok).toBe(true);
  });

  test("forwards click handlers to a nested UI.Button bind target", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/");

    const island = wsIslandAt(page, "ForwardingDemo", 0);
    await island.expectText("count", "0");

    await island.root.getByRole("button", { name: "Reset list" }).click();

    await island.expectText("count", "1");
  });

  test("quotes expression-backed input attributes with spaces", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/", { hydrate: false });

    const input = page.locator("#demo-name");
    await expect(input).toHaveAttribute("placeholder", "Ada Lovelace");
    await expect(input).not.toHaveAttribute("lovelace", "");
  });

  test("renders elseif and switch branches inside UI.Button", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/", { hydrate: false });

    const iconButton = page.getByRole("button", { name: "Icon action" });
    await expect(iconButton).toHaveAttribute("data-variant", "link");
    await expect(iconButton.locator(".ui-button-icon-label")).toHaveText(
      "Icon action",
    );

    const disabledButton = page.getByRole("button", { name: "Unavailable" });
    await expect(disabledButton).toBeDisabled();
    await expect(disabledButton).toHaveAttribute("data-variant", "default");
  });
});
