---
sidebar_position: 1
slug: /intro
title: Overview
---

# Overview

WebScript combines a server scripting language, typed template engine, routing system, action framework, auth layer, response API, and tiny client runtime into one framework.

The intended developer experience is:

```bash
web serve
```

Then edit a `.web` file, refresh the browser, and see the result.

There is no required Vite, bundler, transpile command, asset manifest, or generated route file. The runtime loads `.web` files directly, lazily parses and type-checks them, compiles them to an internal representation, caches the result, and renders responses.

## What WebScript Is For

WebScript is designed for:

- Server-rendered applications.
- Dashboards and admin panels.
- Content sites with forms and authenticated areas.
- API-backed applications that need HTML pages and JSON routes together.
- Serverless and edge deployments where direct file deployment should still work.
- Teams that want PHP-style simplicity with typed request, auth, and response primitives.

## Core Concepts

WebScript keeps common web concepts native:

- Pages are declared with `@page`.
- API routes are declared with `@api`.
- Route data is loaded with `@load`.
- Mutations are handled with `@action`.
- Markup is written directly in the file.
- Sync variables use `@let`.
- Larger sync logic uses `@do`.
- Async HTML uses `@await`.
- Streamed sections use `@defer`.
- Browser-only state uses `@client`.
- Auth is configured with `@auth` and enforced with `@require`.
- Responses are values such as `json(...)`, `redirect(...)`, and `notFound(...)`.

:::warning[Not Yet Implemented]
Several core concepts listed above are documented but not yet implemented in the current MVP runtime:
- `@api` routes — use `@page` routes with `@load` instead
- `@await` / `@loading` / `@error` — use `@defer` with `@placeholder` instead
- `@auth` / `@require` / `@guest` — not yet available
- Response helpers (`json()`, `notFound()`, etc.) — not yet available
- `@query`, `@body`, `@headers` — not yet available

See the individual guide pages for detailed implementation status.
:::

## Example Page

:::warning[Not Yet Implemented]
This example uses `@auth required`, `auth.user`, and typed auth context — these auth features are documented but not yet implemented in the current MVP runtime.
:::

```web
@page "/dashboard"

@auth required {
  redirect: "/login"
}

@load {
  user: User = auth.user
}

<main>
  <h1>Hello {user.name}</h1>

  @defer {
    stats: Stats = await analytics.getStats(user.id)

    <StatsCard stats={stats} />
  } @placeholder {
    <StatsCardSkeleton />
  } @error err {
    <ErrorBox message="Could not load stats" />
  }
</main>
```

This file defines the route, protects it with auth, loads typed data, renders HTML immediately, and streams the slow stats panel later.

## Execution Model

For each request:

1. The runtime resolves a `.web` file or route module.
2. If needed, it parses, type-checks, and compiles the file.
3. The compiled representation is cached.
4. Request context is created, including params, query, headers, cookies, body, session, and auth.
5. Guards run before page logic.
6. `@load` and route-level logic execute.
7. HTML or response values are produced.
8. Deferred blocks may continue resolving and stream replacement HTML.

## No Required Build Step

WebScript does compile internally, but compilation is lazy and runtime-owned. The developer does not need to run a build command before serving, testing, or deploying.

Optional snapshot builds can exist for performance:

:::warning[Not Yet Implemented]
Snapshot mode (`@deploy { mode: "snapshot" }`) and `web snapshot` are documented but not yet implemented. Only runtime mode is currently available.
:::

```web
@deploy {
  mode: "snapshot"
}
```

But the default is runtime mode:

```web
@deploy {
  mode: "runtime"
}
```

