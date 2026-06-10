import { expect, type APIResponse, type Page } from "@playwright/test";
import type { ProjectWorkspace } from "./project-workspace.js";

export async function submitAction(
  page: Page,
  workspace: ProjectWorkspace,
  urlPath: string,
  action: string,
  fields: Record<string, string> = {},
): Promise<void> {
  await workspace.goto(page, urlPath);
  await page.evaluate(
    ({ path, actionName, formFields }) => {
      const form = document.createElement("form");
      form.method = "post";
      form.action = path;

      const actionInput = document.createElement("input");
      actionInput.type = "hidden";
      actionInput.name = "_action";
      actionInput.value = actionName;
      form.appendChild(actionInput);

      for (const [name, value] of Object.entries(formFields)) {
        const input = document.createElement("input");
        input.type = "hidden";
        input.name = name;
        input.value = value;
        form.appendChild(input);
      }

      document.body.appendChild(form);
      form.submit();
    },
    { path: urlPath, actionName: action, formFields: fields },
  );
  await page.waitForLoadState("load");
}

export async function submitActionRaw(
  page: Page,
  workspace: ProjectWorkspace,
  urlPath: string,
  action: string,
  fields: Record<string, string> = {},
): Promise<APIResponse> {
  const body = new URLSearchParams({ _action: action, ...fields });

  return page.request.post(new URL(urlPath, workspace.baseURL).toString(), {
    data: body.toString(),
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    maxRedirects: 0,
  });
}

export async function expectRedirect(
  response: APIResponse,
  location: string,
): Promise<void> {
  expect(response.status()).toBe(303);
  const header = response.headers().location ?? "";
  expect(header).toContain(location);
}
