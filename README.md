# WebScript Framework Documentation

WebScript is a typed, HTML-native scripting language and runtime for building server-rendered web applications with PHP-like immediacy and modern framework safety.

The core design goals are:

- Write pages, APIs, actions, markup, and request logic in `.web` files.
- Use logic directly inside HTML without losing type safety.
- Treat async rendering, auth, sessions, responses, cookies, and server streaming as first-class language/runtime features.
- Require no user-visible build step for development or deployment.
- Allow optional precompilation only as a performance optimization.

## Documentation

Docs are published with [Docusaurus](docs/) under `docs/docs/`.

```bash
cd docs
pnpm install
pnpm start
```

Start here:

- [Overview](docs/docs/intro.md)
- [Tutorial: build a web app](docs/docs/tutorial/01-project-setup.md)

**Language**

- [Syntax](docs/docs/language/syntax.md)
- [Control Flow](docs/docs/language/control-flow.md)
- [Type System](docs/docs/language/type-system.md)
- [HTML Templates](docs/docs/language/html-templates.md)
- [Directives](docs/docs/language/directives.md)
- [Server Logic](docs/docs/language/server-logic.md)

**Standard library**

- [Standard Helpers](docs/docs/stdlib/standard-helpers.md)

**Guides**

- [Getting Started](docs/docs/guides/getting-started.md)
- [Project Structure](docs/docs/guides/project-structure.md)
- [CLI](docs/docs/guides/cli.md)
- [Configuration](docs/docs/guides/configuration.md)
- [Components](docs/docs/guides/components.md)
- [Layouts](docs/docs/guides/layouts.md)
- [Routing](docs/docs/guides/routing.md)
- [Data Loading And Actions](docs/docs/guides/data-loading-and-actions.md)
- [Forms And Validation](docs/docs/guides/forms-and-validation.md)
- [Auth And Sessions](docs/docs/guides/auth-and-sessions.md)
- [Requests And Responses](docs/docs/guides/requests-and-responses.md)
- [API Routes](docs/docs/guides/api-routes.md)
- [Client Interactivity](docs/docs/guides/client-interactivity.md)
- [Async Rendering](docs/docs/guides/async-rendering.md)
- [Styling And Assets](docs/docs/guides/styling-and-assets.md)
- [Runtime And Deployment](docs/docs/guides/runtime-and-deployment.md)
- [Error Handling](docs/docs/guides/error-handling.md)
- [Security](docs/docs/guides/security.md)
- [Examples](docs/docs/guides/examples.md)

## Design Rule

Build steps are allowed as an optimization, never as a requirement.

## MVP Status

This repository now includes a small Rust MVP for the `web` binary.

Supported today:

- `web new <name>` creates a starter app.
- `web routes` discovers explicit `@page` routes under `app/`.
- `web check` parses `.web` files and validates template bindings, simple `@if` conditions, and `@for` loops.
- `web serve --port 3000` serves pages directly from `.web` files.
- `@load` and `@action` server blocks with `fn`, `while`, `try/catch`, `throw`, `await`, `sleep`, `spawn`, `timeout`, and `fetch`.
- `@do` sync server blocks (no `await`/`fetch`).

The first supported language slice includes template expressions, bool `@if` blocks, array `@let` values, scoped `@for` loops, typed component props, simple `string`/`int` route params, and server logic documented in [Server Logic](docs/docs/language/server-logic.md):

```web
@page "/"

@let name = "WebScript"
@let visits = 2 + 3
@let greeting = "Hello " + name
@let posts: string[] = ["One", "Two", "Three"]

<main>
  <h1>{greeting}</h1>
  <p>Visit count: {visits + 1}</p>

  @if visits > 3 {
    <p>Direct .web serving is alive.</p>
  }

  @for post in posts {
    <PostPreview title={post} featured={visits > 3} />
  }
</main>
```

Run the sample:

```bash
cargo run --bin web -- serve
```
