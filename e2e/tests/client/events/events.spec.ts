import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("@client events and modifiers", () => {
  test.beforeEach(async ({ page, workspace }) => {
    await open(page, workspace, "/");
  });

  test("@submit.prevent, @keydown, @focus, @blur", async ({ page }) => {
    const island = wsIslandAt(page, "EventDemo", 0);
    const root = island.root;

    await root.getByRole("button", { name: "Save (@submit.prevent)" }).click();
    await expect(island.branch("submitted", "then")).toBeVisible();
    await expect(root.locator(".event-success")).toHaveText("Saved");

    const keys = root.locator("#event-keys");

    await keys.focus();
    await expect(root.locator(".event-success")).toHaveText("focused");

    await keys.press("a");
    await island.expectText("keyCount", "1");

    await keys.blur();
    await expect(root.locator(".event-success")).toHaveText("blurred");

    await keys.focus();
    await keys.press("Enter");
    await expect(root.locator(".event-success")).toHaveText("Saved");
  });
});
