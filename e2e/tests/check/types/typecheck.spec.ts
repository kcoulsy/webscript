import * as path from "node:path";
import { test, type TestInfo } from "@playwright/test";
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

test.describe("type checking (web check)", () => {
  test("valid colocated project passes", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();
    await workspace.dispose();
  });

  test("rejects component prop type mismatch", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-prop-type.web",
      '@page "/bad-prop-type"\n\n<main>\n<Metric value={true} />\n</main>\n',
    );
    await workspace.assertCheckFails(/expects `int`/);

    await workspace.dispose();
  });

  test("rejects missing required prop", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-missing-prop.web",
      '@page "/bad-missing-prop"\n\n<main>\n<Box />\n</main>\n',
    );
    await workspace.assertCheckFails(/missing prop/);

    await workspace.dispose();
  });

  test("rejects unknown component", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-unknown-component.web",
      '@page "/bad-unknown-component"\n\n<main>\n<Ghost />\n</main>\n',
    );
    await workspace.assertCheckFails(/unknown component/);

    await workspace.dispose();
  });

  test("rejects unknown prop", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-unknown-prop.web",
      '@page "/bad-unknown-prop"\n\n<main>\n<Counter initial={0} label="Ok" extra="x" />\n</main>\n',
    );
    await workspace.assertCheckFails(/unknown prop/);

    await workspace.dispose();
  });

  test("rejects @let type mismatch", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-let.web",
      '@page "/bad-let"\n\n@let n: int = "x"\n\n<main>{n}</main>\n',
    );
    await workspace.assertCheckFails(/expects `int`/);

    await workspace.dispose();
  });

  test("accepts string literal union props", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/components/Button.web",
      '@component Button {\n  variant: "primary" | "secondary" = "primary"\n  kind: "icon" = "icon"\n}\n\n<button>{variant}</button>\n',
    );
    await workspace.write(
      "app/pages/literal-union.web",
      '@page "/literal-union"\n\n<main>\n<Button variant="secondary" kind="icon" />\n<Button />\n</main>\n',
    );
    await workspace.assertCheckOk();

    await workspace.dispose();
  });

  test("rejects string literal union prop mismatch", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/components/Button.web",
      '@component Button {\n  variant: "primary" | "secondary" = "primary"\n}\n\n<button>{variant}</button>\n',
    );
    await workspace.write(
      "app/pages/bad-literal-union.web",
      '@page "/bad-literal-union"\n\n<Button variant="ghost" />\n',
    );
    await workspace.assertCheckFails(/expects `"primary" \| "secondary"`/);

    await workspace.dispose();
  });

  test("rejects string literal union default mismatch", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/components/BadButton.web",
      '@component BadButton {\n  variant: "primary" | "secondary" = "ghost"\n}\n\n<button>{variant}</button>\n',
    );
    await workspace.assertCheckFails(/expected `"primary" \| "secondary"`/);

    await workspace.dispose();
  });

  test("rejects @if non-bool condition", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-if.web",
      '@page "/bad-if"\n\n@let flag: string = "yes"\n\n@if flag {\n  <p>yes</p>\n}\n',
    );
    await workspace.assertCheckFails(/must be bool/);

    await workspace.dispose();
  });

  test("rejects @for non-array source", async ({}, testInfo) => {
    const workspace = await createWorkspace(testInfo);
    await workspace.assertCheckOk();

    await workspace.write(
      "app/pages/bad-for.web",
      '@page "/bad-for"\n\n@let n: int = 5\n\n@for item in n {\n  <p>{item}</p>\n}\n',
    );
    await workspace.assertCheckFails(/must be array/);

    await workspace.dispose();
  });
});
