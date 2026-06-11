import * as path from "node:path";
import { expect, test, type TestInfo } from "@playwright/test";
import {
  colocatedProjectDir,
  findRepoRoot,
  ProjectWorkspace,
} from "../../../framework/project-workspace.js";

async function createWorkspace(testInfo: TestInfo) {
  const workspace = await ProjectWorkspace.create({
    repoRoot: findRepoRoot(path.dirname(testInfo.file)),
    sourceDir: colocatedProjectDir(testInfo.file),
  });
  await workspace.materialize();
  return workspace;
}

test.describe("schema validation (web check)", () => {
  test("valid colocated project passes", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();
    await workspace.dispose();
  });

  test("rejects unsupported schema field type", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/schemas/BadType.web",
      "@schema BadType {\n  id: uuid\n}\n",
    );
    await workspace.assertCheckFails(/unsupported schema type/);

    await workspace.dispose();
  });

  test("rejects duplicate schema field", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/schemas/Duplicate.web",
      "@schema Duplicate {\n  name: string\n  name: string\n}\n",
    );
    await workspace.assertCheckFails(/duplicate field/);

    await workspace.dispose();
  });

  test("rejects fetch without schema argument", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/fetch-bad.web",
      '@page "/fetch-bad"\n\n@load {\n  _: object = await fetch("https://example.com")\n}\n\n<main></main>\n',
    );
    await workspace.assertCheckFails(/2 arguments/);

    await workspace.dispose();
  });

  test("rejects fetch with unknown schema", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/fetch-unknown.web",
      '@page "/fetch-unknown"\n\n@load {\n  _: object = await fetch("https://example.com", Missing)\n}\n\n<main></main>\n',
    );
    await workspace.assertCheckFails(/unknown schema/);

    await workspace.dispose();
  });

  test("rejects db.query without schema argument", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/db-bad.web",
      '@page "/db-bad"\n\n@load {\n  _: object = await db.query("SELECT 1")\n}\n\n<main></main>\n',
    );
    await workspace.assertCheckFails(/requires a schema/);

    await workspace.dispose();
  });

  test("rejects db.query with unknown schema", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/db-unknown.web",
      '@page "/db-unknown"\n\n@load {\n  _: object = await db.query("SELECT 1", Missing)\n}\n\n<main></main>\n',
    );
    await workspace.assertCheckFails(/unknown schema/);

    await workspace.dispose();
  });
});
