# Configuration

WebScript configuration can live in `web.config` and route/component files through top-level directives. Runtime defaults should be secure and useful without large configuration files.

## `web.config`

Example:

```web
@deploy {
  mode: "runtime"
  adapter: "node"
}

@auth {
  mode: "stateful-session"
  store: kv("sessions")
  cookie: "__Host_app_session"
  sameSite: "lax"
  secure: true
  httpOnly: true
  ttl: 30d
}

@session {
  store: kv("sessions")
  ttl: 30d

  data {
    userId: UserId
    roles: string[]
    csrfToken: string
  }
}
```

## Environment

Read environment values with `env(...)`:

```web
databaseUrl: string = env("DATABASE_URL")
sessionSecret: string = env("SESSION_SECRET")
```

Secrets must stay server-only and must not be passed to `@client`.

## Deployment Mode

Runtime mode:

```web
@deploy {
  mode: "runtime"
}
```

Snapshot mode:

```web
@deploy {
  mode: "snapshot"
}
```

Runtime mode ships source files and compiles lazily. Snapshot mode precompiles as an optimization.

## Adapters

Adapters define where the app runs:

```web
@deploy {
  adapter: "node"
}
```

Potential adapters:

```web
"node"
"serverless"
"edge"
"cloudflare"
"vercel"
"deno"
```

Adapter names are runtime-defined.

## Auth Defaults

Recommended default:

```web
@auth {
  mode: "stateful-session"
  store: kv("sessions")
}
```

This stores only a signed opaque session ID in the cookie and keeps user/session data server-side.

## Global Headers

```web
@headers global {
  "X-Content-Type-Options": "nosniff"
  "Referrer-Policy": "strict-origin-when-cross-origin"
}
```

Route-level headers can add or override global headers.

## Feature Flags

```web
@features {
  streaming: true
  clientRuntime: true
  scopedCss: true
}
```

Feature flags should be explicit when they affect generated output or runtime behavior.

## Cache Settings

```web
@cache {
  compiledRoutes: ".web/cache/routes"
  ttl: 1h
}
```

In serverless environments, the adapter may map this to platform cache primitives.

