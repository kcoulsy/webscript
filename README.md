# WebScript Framework Documentation

WebScript is a typed, HTML-native scripting language and runtime for building server-rendered web applications with PHP-like immediacy and modern framework safety.

The core design goals are:

- Write pages, APIs, actions, markup, and request logic in `.web` files.
- Use logic directly inside HTML without losing type safety.
- Treat async rendering, auth, sessions, responses, cookies, and server streaming as first-class language/runtime features.
- Require no user-visible build step for development or deployment.
- Allow optional precompilation only as a performance optimization.

## Documentation

Start here:

- [Overview](docs/overview.md)
- [Getting Started](docs/getting-started.md)
- [Project Structure](docs/project-structure.md)
- [CLI](docs/cli.md)
- [Configuration](docs/configuration.md)
- [Language Syntax](docs/language-syntax.md)
- [Directive Reference](docs/directive-reference.md)
- [HTML Templates](docs/html-templates.md)
- [Control Flow](docs/control-flow.md)
- [Async Rendering](docs/async-rendering.md)
- [Components](docs/components.md)
- [Layouts](docs/layouts.md)
- [Routing](docs/routing.md)
- [Data Loading And Actions](docs/data-loading-and-actions.md)
- [Forms And Validation](docs/forms-and-validation.md)
- [Auth And Sessions](docs/auth-and-sessions.md)
- [Requests And Responses](docs/requests-and-responses.md)
- [API Routes](docs/api-routes.md)
- [Client Interactivity](docs/client-interactivity.md)
- [Styling And Assets](docs/styling-and-assets.md)
- [Runtime And Deployment](docs/runtime-and-deployment.md)
- [Type System](docs/type-system.md)
- [Error Handling](docs/error-handling.md)
- [Security](docs/security.md)
- [Standard Helpers](docs/standard-helpers.md)
- [Examples](docs/examples.md)

## Design Rule

Build steps are allowed as an optimization, never as a requirement.

## MVP Status

This repository now includes a small Rust MVP for the `web` binary.

Supported today:

- `web new <name>` creates a starter app.
- `web routes` discovers explicit `@page` routes under `app/`.
- `web check` parses `.web` files and validates simple template bindings.
- `web serve --port 3000` serves pages directly from `.web` files.

The first supported language slice is intentionally small:

```web
@page "/"

@let name: string = "WebScript"

<main>
  <h1>Hello {name}</h1>
</main>
```

Run the sample:

```bash
cd examples/hello
cargo run --bin web -- serve
```
