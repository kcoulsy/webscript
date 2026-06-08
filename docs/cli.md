# CLI

The WebScript CLI is named `web`. It should keep the common workflow small while exposing enough commands for production and CI.

## `web new`

Create a project:

```bash
web new my-app
```

Expected output:

```txt
my-app/
  app/
  public/
  styles/
  web.config
```

## `web serve`

Start the development server:

```bash
web serve
```

The server:

- Loads `.web` files directly.
- Lazily compiles files on first request.
- Watches source files.
- Invalidates cached compiled routes after edits.
- Serves `public` files.
- Serves `/.web/runtime.js`.
- Shows development errors with source locations.

Optional host and port:

```bash
web serve --host 0.0.0.0 --port 3000
```

## `web check`

Type-check the project without starting a server:

```bash
web check
```

This command should validate:

- Route declarations.
- Component props.
- Action input types.
- API response types.
- Auth/session configuration.
- Client/server boundaries.
- Nullable access.

## `web routes`

Print the route table:

```bash
web routes
```

Example:

```txt
GET   /                  app/pages/index.web
GET   /dashboard         app/pages/dashboard.web
POST  /actions/login     app/pages/login.web@login
GET   /api/posts         app/api/posts.web
POST  /api/posts         app/api/posts.web
```

## `web snapshot`

Create an optional precompiled snapshot:

```bash
web snapshot
```

Snapshot output may include:

- Compiled route modules.
- Route manifest.
- Extracted scoped CSS.
- Fingerprinted assets.
- Runtime cache metadata.

Snapshot builds are never required for normal development.

## `web deploy`

Deploy using the configured adapter:

```bash
web deploy
```

Deployment behavior is controlled by:

```web
@deploy {
  mode: "runtime"
}
```

or:

```web
@deploy {
  mode: "snapshot"
}
```

## `web doctor`

Inspect local environment and project health:

```bash
web doctor
```

This should check:

- Runtime version.
- Adapter availability.
- Session store connectivity.
- Environment variables.
- Writable cache paths.
- Route conflicts.

## Command Philosophy

The common path should stay:

```bash
web serve
```

Other commands exist for confidence, CI, diagnostics, and optimization.

