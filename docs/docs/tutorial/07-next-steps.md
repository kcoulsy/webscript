---
sidebar_position: 7
title: Next steps
---

# Next steps

You now have a working mental model of WebScript:

1. **Pages** — `@page` routes with markup and `{expressions}`
2. **Components** — typed, reusable UI in `app/components/`
3. **Data** — `@load` for reads, `@action` for mutations
4. **Routing** — dynamic segments, query params, early `notFound()`
5. **Server logic** — `fn`, `while`, `try/catch`, `fetch`, `sleep`, `spawn`, `timeout`
6. **Control flow** — `@if`, `@for`, `@do` in templates

## Try the sample app in this repo

This repository includes a demo app under `app/`:

```bash
cargo run --bin web -- serve --port 3000
```

| Route | Features |
|-------|----------|
| `/` | `@let`, `@do`, `@if`, `@for`, `@action`, session, components |
| `/posts/{slug}` | Dynamic params, `@load` |
| `/async-demo` | `fn`, `while`, `sleep`, `spawn`, `timeout` |
| `/fetch-demo` | `fetch`, `throw`, `try/catch` |

## Go deeper

### Language reference

- [Syntax](../language/syntax) — types, literals, expressions
- [Control Flow](../language/control-flow) — `@if`, `@for`, `@await`, `@defer`
- [Type System](../language/type-system)
- [Directives](../language/directives) — full `@` directive list
- [Server Logic](../language/server-logic)

### Standard library

- [Standard Helpers](../stdlib/standard-helpers) — `json`, `redirect`, `auth`, `log`, and more

### Guides

- [API Routes](../guides/api-routes) — JSON endpoints with `@api`
- [Auth and Sessions](../guides/auth-and-sessions)
- [Layouts](../guides/layouts) — shared HTML shells
- [Client Interactivity](../guides/client-interactivity) — `@client` for browser state
- [Runtime and Deployment](../guides/runtime-and-deployment)

## Design principle

WebScript compiles internally, but **you never need a build step** to develop or deploy. Run `web serve`, edit `.web` files, refresh — that is the intended workflow.
