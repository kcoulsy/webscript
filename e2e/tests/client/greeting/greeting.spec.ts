import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("@client @input and signal<string>", () => {
  test.beforeEach(async ({ page, workspace }) => {
    await open(page, workspace, "/");
  });

  test("two-way value binding updates reactive text", async ({ page }) => {
    const root = wsIslandAt(page, "Greeting", 0).root;
    const input = root.locator('[data-ws-value="name"]');
    const output = root.locator('[data-ws-text="name"]');

    await expect(output).toHaveText("");
    await input.fill("Ada");
    await expect(output).toHaveText("Ada");
  });
});
