---
sidebar_position: 2
slug: /implementation-status
title: Implementation Status
---

# Implementation Status

This page tracks which documented features are available in the current MVP runtime and which are still planned. Features marked with **Not Yet Implemented** are documented as design targets but do not yet exist in the runtime.

## Currently Implemented

These features work in the current MVP:

### Language & Templates
- `@page` route declarations
- `@let` variable declarations (type inference and explicit types)
- `@if` / `@else if` / `@else` conditional rendering
- `@for` loops (without index or `@empty`)
- `@do` synchronous server blocks
- `@switch` / `@case` / `@default` conditional switching
- `@defer` with `@placeholder` and `@error` for streamed sections
- `@style` / `@style scoped` / `@style global` scoped CSS
- `@component` with typed props (string, int, bool, string literal unions)
- `@layout` with props and `<slot />` (default slot only)
- `@defaults { layout }` configuration
- `@client` block with signals and named handlers
- Event handlers: `@click`, `@input`, `@change`, `@submit`, `@keydown`, `@keyup`, `@focus`, `@blur`
- Event modifiers: `.prevent`, `.stop`, `.prevent.stop`
- Pipe-lambda, named handler, and bare expression handler syntax
- Reactive `@if` driven by `signal<bool>`
- Comments (`//` and `/* */`)
- String, int, bool, float, object, array, duration literals
- String interpolation in attributes
- Boolean attributes
- Fragment rendering (multiple sibling nodes)
- Scoped CSS via `data-ws-style`

### Server Logic
- `@load` blocks (async, full server syntax)
- `@action` blocks (session mutations and redirects)
- `fn` named functions in server blocks
- `if` / `while` / `try` / `catch` / `throw` in server blocks
- `fail("message")` for validation failures
- `redirect("/")` for action redirects
- `:=` type inference in server blocks
- `await sleep(duration)`, `await fetch(url, Schema)`, `spawn(promise)`, `timeout(duration, promise)`

### Components & Layouts
- Self-closing and children component calls
- Component event forwarding via `data-ws-bind`
- Component class passthrough
- Namespaced/dotted components (`UI.Button`)
- Layout rendering with `<slot />`

### Database
- `@model` definition (field types, `@primary`, `@auto`, `@unique`, `@nullable`, `@default`, `@references`, `@relation`, `@index`, `@uniqueIndex`)
- `web db:generate` and `web db:migrate` CLI commands
- Model helpers: `all()`, `find()`, `create()`, `update()`, `deleteAll()`, `where({})`, `count()`
- Raw SQL: `db.query(sql, Schema)`, `db.query(sql, params, Schema)`, `db.execute(sql, params?)`

### Schemas & Validation
- `@schema` definition (field types, `@min`, `@max`, `@email`, `@optional`)
- Runtime schema validation for `fetch()` and `db.query()`
- `web check` validates schema references

### Client Runtime
- Per-island hydration (`/.web/runtime.js`)
- Signal system (`signal<int>`, `signal<bool>`, `signal<string>`)
- Handler compilation (increment, decrement, toggle, assignment, method calls)
- `WebScript.action()` for client-to-server action calls
- `WebScript.defer.replace()` for streamed deferred sections
- Hot reload via WebSocket in dev mode

### Dev Tools
- `web serve` dev server (lazy compile, hot reload, Tailwind CSS)
- `web routes` route discovery
- `web check` parse and validate
- Debug bar at `/.web/debug`
- Session-based `@action` with `fail()` and `redirect()`
- In-memory session (strings only via `webscript_session` cookie)
- `crypto.hashPassword()` / `crypto.verifyPassword()` (Argon2id)
- Tailwind CSS on-demand compilation
- Static file serving from `public/`
- `@deploy` and `@tailwind` config in `web.config`

---

## Not Yet Implemented

The following features are documented but not yet available in the runtime.

### Routing
| Feature | Description |
|---------|-------------|
| `@api` routes | API/JSON route declarations — only `@page` routes are available |
| `@query { }` | Typed query parameter parsing and validation |
| `@body input: TypeName` | Typed request body parsing for actions and APIs |
| `@headers { }` | Response header declarations on routes |
| Route guards | `@require auth`, `@require role(...)`, `@require anyRole(...)`, `@require can(...)` |
| `@auth required { redirect }` | Auth-required route protection with redirect |
| `@guest { redirect }` | Guest-only route restriction |
| Custom scalar route params | Route segments like `{id:UserId}` with custom types |

### Auth & Sessions
| Feature | Description |
|---------|-------------|
| `@auth { }` configuration | Auth driver, mode, cookie, TTL configuration |
| `@session { data { } }` | Typed session declarations |
| `auth.login(userId)` / `auth.logout()` | Login and logout helpers |
| `auth.check`, `auth.user`, `auth.userId`, `auth.roles` | Auth context properties |
| Session cookie signing | Signed opaque session IDs |
| Session rotation | ID rotation, TTL refresh, CSRF regeneration on login |
| CSRF protection | `@csrf` token generation, verification, rotation |
| `__Host_` cookie prefix | Secure session cookie naming |

### Templates & Control Flow
| Feature | Description |
|---------|-------------|
| `@await` / `@loading` / `@error` | Three-state async rendering in markup — use `@defer` instead |
| `@async` / `@then` / `@loading` / `@error` | Verbose async block in markup |
| `@for` with index | `@for item, index in items` — zero-based index variable |
| `@for @empty` | Empty-state branch on loops |
| Named slots | `<slot name="..." />` and `<template slot="...">` — only default `<slot />` is available |
| Dynamic class objects | `class={{ "active": isActive }}` syntax |
| `html.trusted()` | Explicit trusted/raw HTML rendering |

### Types & Validation
| Feature | Description |
|---------|-------------|
| `date`, `datetime`, `bytes` | Primitive types not yet supported |
| `Promise<T>` | Promise type annotation |
| Response types | `Json<T>`, `Redirect`, `Html`, `Response` |
| Custom scalar types | `type UserId scalar string` |
| Input validation annotations | `@email`, `@min`, `@max` inside `type` blocks — use `@schema` instead |
| Structured `fail()` | `fail({ field: "message" })` — only `fail("message")` string form available |
| `form.errors.fieldName` | Template access to field-level validation errors |
| `form.pending` | Form submission pending state |
| Type checking | Full type enforcement (missing props, wrong types, nullable access, server/client boundaries) |

### Forms & Actions
| Feature | Description |
|---------|-------------|
| `<form @submit={actionName}>` | Declarative form-action binding — use `<form method="post">` with `_action` hidden field |
| `@body input: TypeName` | Typed action input parsing |
| `@csrf` | CSRF token injection |
| File uploads | `File` type, `@maxSize`, `@mime` validation |

### Request & Response
| Feature | Description |
|---------|-------------|
| `request` context | `request.method`, `.url`, `.path`, `.params`, `.query`, `.headers`, `.cookies`, `.body`, `.ip`, `.userAgent` |
| Response helpers | `response()`, `html()`, `json()`, `ok()`, `created()`, `noContent()`, `badRequest()`, `unauthorized()`, `forbidden()`, `notFound()`, `conflict()`, `unprocessable()`, `serverError()` |
| Response modifiers | `.status()`, `.header()`, `.cookie()`, `.signedCookie()`, `.forgetCookie()` |
| `notFound()` | Helper to return 404 responses from `@load` |

### Standard Helpers
| Feature | Description |
|---------|-------------|
| `slug()` | String slugify helper |
| `escape()` | HTML escape helper |
| `asset()` | Asset path resolver with fingerprinting |
| `formatDate()`, `now()` | Date helpers |
| `log.debug/info/warn/error()` | Logging helpers |
| `rateLimit()` | Rate limiting helper |
| `kv()` | Key-value store |
| `storage.put()` / `storage.get()` | File storage helpers |
| `env()` | Environment variable access |
| `role()`, `anyRole()`, `can()`, `require()` | Guard helpers |
| `request.header/cookie/accepts/ip/userAgent` | Request access helpers |
| `db.transaction { }` | Database transaction wrapper |
| `Model.findByEmail()` | Additional model finder methods |

### Security
| Feature | Description |
|---------|-------------|
| Secure default headers | `X-Frame-Options`, `X-Content-Type-Options`, `Referrer-Policy`, CSP |
| `@headers` | Per-route and global response headers |
| `env("VAR")` | Environment variable access |
| `rateLimit()` | Rate limiting |

### Deployment & CLI
| Feature | Description |
|---------|-------------|
| `web snapshot` | Precompiled snapshot creation |
| `web deploy` | Deployment with configured adapter |
| `web doctor` | Environment and project health inspection |
| Snapshot mode | `@deploy { mode: "snapshot" }` |
| Deployment adapters | node, serverless, edge, cloudflare, vercel, deno |

### Client Interactivity
| Feature | Description |
|---------|-------------|
| Reactive `@for` | Client-side reactive list rendering |
| Page-level `@client` | `@client` blocks on pages (currently component-only) |
| Enhanced forms | `form.pending`, `form.errors`, progressive enhancement without full reload |