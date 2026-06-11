import * as path from "node:path";
import { test as base, expect } from "@playwright/test";
import {
  colocatedProjectDir,
  findRepoRoot,
  ProjectWorkspace,
} from "../../../framework/project-workspace.js";
import {
  expectRedirect,
  submitActionRaw,
} from "../../../framework/server-actions.js";
import { open } from "../../../framework/ws.js";

type DbFixtures = {
  workspace: ProjectWorkspace;
};

const test = base.extend<DbFixtures>({
  workspace: async ({}, use, testInfo) => {
    const workspace = await ProjectWorkspace.create({
      repoRoot: findRepoRoot(path.dirname(testInfo.file)),
      sourceDir: colocatedProjectDir(testInfo.file),
    });
    await workspace.materialize();
    await workspace.setupDatabase("init");
    await workspace.assertCheckOk();
    await workspace.startServer(testInfo.parallelIndex);
    await use(workspace);
    await workspace.dispose();
  },
});

test.describe("schema validation (runtime)", () => {
  test("fetch with valid schema populates template", async ({ page, workspace }) => {
    await open(page, workspace, "/fetch-ok", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Fetch OK");
    await expect(page.getByText(/^Title:/)).toContainText("delectus");
    await expect(page.getByText(/^Error:\s*$/)).toBeVisible();
  });

  test("fetch schema mismatch surfaces in try/catch", async ({ page, workspace }) => {
    await open(page, workspace, "/fetch-bad", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("Fetch bad");
    await expect(page.getByText(/^Error:/)).toContainText("extraRequired");
  });

  test("db.query returns typed rows", async ({ page, workspace }) => {
    await open(page, workspace, "/db-ok", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("DB OK");
    await expect(page.getByText(/^Query OK:/)).toContainText("true");
    await expect(page.getByText(/^Error:\s*$/)).toBeVisible();
  });

  test("db.query row/schema mismatch surfaces in try/catch", async ({
    page,
    workspace,
  }) => {
    await open(page, workspace, "/db-bad", { hydrate: false });
    await expect(page.locator("main h1")).toHaveText("DB bad");
    await expect(page.getByText(/^Error:/)).toContainText("done");
  });

  test("@action schema rejects empty input", async ({ page, workspace }) => {
    const response = await submitActionRaw(
      page,
      workspace,
      "/action",
      "submitItem",
      { title: "" },
    );
    expect(response.status()).toBe(500);
    await expect(response.text()).resolves.toMatch(/title/i);
  });

  test("@action schema accepts valid input", async ({ page, workspace }) => {
    const response = await submitActionRaw(
      page,
      workspace,
      "/action",
      "submitItem",
      { title: "Ada" },
    );
    await expectRedirect(response, "/action");

    await open(page, workspace, "/action", { hydrate: false });
    await expect(page.getByText("Last title: Ada")).toBeVisible();
  });
});

export { expect, test };
