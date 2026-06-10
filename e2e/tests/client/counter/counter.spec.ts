import { expect, test } from "../../../framework/fixture.js";
import { open, wsIslandAt } from "../../../framework/ws.js";

test.describe("@client signals and @click", () => {
  test.beforeEach(async ({ page, workspace }) => {
    await open(page, workspace, "/counter");
  });

  test("colocated project passes web check", async ({ workspace }) => {
    expect((await workspace.check()).ok).toBe(true);
  });

  test("signal<int> updates independently per island", async ({ page }) => {
    const score = wsIslandAt(page, "Counter", 0);
    const lives = wsIslandAt(page, "Counter", 1);

    await score.expectText("count", "5");
    await lives.expectText("count", "0");

    await score.clickHandler("click", 0).click();
    await score.expectText("count", "6");

    await score.clickHandler("click", 1).click();
    await score.expectText("count", "5");

    await score.clickHandler("click", 2).click();
    await score.expectText("count", "0");

    await lives.clickHandler("click", 0).click();
    await lives.expectText("count", "1");
    await score.expectText("count", "0");
  });
});
