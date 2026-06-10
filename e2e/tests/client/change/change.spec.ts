import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("@client @change", () => {
  test("updates signal from change event", async ({ page, workspace }) => {
    await open(page, workspace, "/");
    const island = wsIslandAt(page, "ChangeField", 0);

    await island.root.locator("#change-field").selectOption("draft");

    await island.expectText("value", "draft");
    await expect(island.root.getByTestId("output")).toHaveText("draft");
  });
});
