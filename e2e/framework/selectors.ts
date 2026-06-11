export interface IslandQuery {
  component: string;
  index: number;
  /** Page route used to build the full island id (e.g. `/counter`). */
  route?: string;
}

function islandRouteScope(route: string): string {
  const trimmed = route.replace(/^\//, "");
  return trimmed.length === 0 ? "_root" : trimmed.replace(/\//g, "_");
}

export function islandSelector({ component, index, route }: IslandQuery): string {
  if (route) {
    const scope = islandRouteScope(route);
    return `[data-ws-island="${scope}-${component}-${index}"]`;
  }
  return `[data-ws-island$="-${component}-${index}"]`;
}

export function signalTextSelector(signal: string): string {
  return `[data-ws-text="${signal}"]`;
}

export function signalValueSelector(signal: string): string {
  return `[data-ws-value="${signal}"]`;
}

export function eventHandlerSelector(
  event: string,
  index: number,
): string {
  return `[data-ws-${event}="${index}"]`;
}

export function signalBranchSelector(
  signal: string,
  branch: "then" | "else",
): string {
  return `[data-ws-if="${signal}"][data-ws-branch="${branch}"]`;
}
