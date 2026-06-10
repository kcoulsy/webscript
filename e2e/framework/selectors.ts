export interface IslandQuery {
  component: string;
  index: number;
}

export function islandSelector({ component, index }: IslandQuery): string {
  return `[data-ws-island="${component}-${index}"]`;
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
