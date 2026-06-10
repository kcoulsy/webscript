import * as path from "node:path";
import { expect, test } from "@playwright/test";
import {
  colocatedProjectDir,
  findRepoRoot,
  ProjectWorkspace,
} from "../../framework/project-workspace.js";

test.describe("web check", () => {
  test("valid colocated project passes", async ({}, testInfo) => {
    const workspace = await ProjectWorkspace.create({
      repoRoot: findRepoRoot(path.dirname(testInfo.file)),
      sourceDir: colocatedProjectDir(testInfo.file),
    });
    await workspace.materialize();
    await workspace.assertCheckOk();
    await workspace.dispose();
  });

  test("detects parse errors in written files", async ({}, testInfo) => {
    const workspace = await ProjectWorkspace.create({
      repoRoot: findRepoRoot(path.dirname(testInfo.file)),
      sourceDir: colocatedProjectDir(testInfo.file),
    });
    await workspace.materialize();
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/broken.web",
      '@page "/broken"\n\n@client {\n  x: signal<int> =\n}\n\n<main></main>\n',
    );
    await workspace.assertCheckFails();

    await workspace.dispose();
  });

  test("detects validation errors after write", async ({}, testInfo) => {
    const workspace = await ProjectWorkspace.create({
      repoRoot: findRepoRoot(path.dirname(testInfo.file)),
      sourceDir: colocatedProjectDir(testInfo.file),
    });
    await workspace.materialize();
    await workspace.assertCheckOk();

    await workspace.write(
      "app/components/Bad.web",
      '@component Bad {}\n\n@client { x: signal<int> = "not an int" }\n\n<div>{x}</div>\n',
    );
    const result = await workspace.assertCheckFails();
    expect(result.output.length).toBeGreaterThan(0);

    await workspace.dispose();
  });
});
