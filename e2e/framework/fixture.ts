import * as path from "node:path";
import { test as base } from "@playwright/test";
import {
  colocatedProjectDir,
  findRepoRoot,
  ProjectWorkspace,
} from "./project-workspace.js";

type FrameworkFixtures = {
  /** Colocated `.web` project copied to a temp dir, checked, and served per test. */
  workspace: ProjectWorkspace;
};

export const test = base.extend<FrameworkFixtures>({
  workspace: async ({}, use, testInfo) => {
    const workspace = await ProjectWorkspace.create({
      repoRoot: findRepoRoot(path.dirname(testInfo.file)),
      sourceDir: colocatedProjectDir(testInfo.file),
    });
    await workspace.materialize();
    await workspace.assertCheckOk();
    await workspace.startServer(testInfo.parallelIndex);
    await use(workspace);
    await workspace.dispose();
  },
});

export { expect } from "@playwright/test";
