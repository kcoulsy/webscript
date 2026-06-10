# WebScript E2E Tests

End-to-end tests for the **WebScript language and runtime** — not the sample `app/` in the repo root.

Each spec owns a colocated `project/` directory of `.web` files. The framework copies that project to a temp workspace, runs `web check`, serves it, and drives the browser through runtime primitives (`data-ws-*` islands, signals, `@action`, routing).

## Layout

```
e2e/
  framework/     Language/runtime helpers (workspace, islands, web check, ws.open)
  runner/        Playwright config + cargo build (swappable)
  tests/
    client/counter/
      counter.spec.ts
      project/         ← .web sources live next to the spec
    check/
      syntax.spec.ts
      project/
```

## Setup

```bash
cd e2e
npm install
npx playwright install chromium
```

## Run

```bash
npm test
```

Headed:

```bash
npm run test:headed
```

## Framework API

| Helper | Purpose |
|--------|---------|
| `workspace` fixture | Materialize colocated `project/`, `web check`, serve |
| `workspace.write(path, src)` | Write `.web` files and re-validate |
| `open(page, workspace, route)` | Navigate; auto-waits for hydration when islands exist |
| `wsIslandAt(page, "Counter", 0)` | Locate `data-ws-island` + signal/event bindings |
| `submitAction` / `submitActionRaw` | POST `@action` forms |

Tests import from `framework/` only — never from `runner/`.
