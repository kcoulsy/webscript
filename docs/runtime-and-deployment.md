# Runtime And Deployment

WebScript has no required user-visible build step. The runtime is responsible for loading, compiling, caching, and rendering `.web` files.

## Development Runtime

Development flow:

```txt
request comes in
runtime loads .web file
parses/compiles if needed
caches compiled bytecode/function
renders response
```

Command:

```bash
web serve
```

The developer edits files and refreshes the browser.

## Lazy Compilation

First request:

```txt
first request to /users
  parse users.web
  type-check users.web
  compile to runtime representation
  cache it
```

Next request:

```txt
next request to /users
  use cached version
```

The runtime should invalidate the cache when a source file changes.

## Production Runtime

Production can still run in runtime mode:

```web
@deploy {
  mode: "runtime"
}
```

Cold start:

```txt
load pre-warmed cache if available
otherwise compile on first hit
```

This keeps deployment simple: ship source files and the runtime.

## Snapshot Mode

Optional precompilation:

```web
@deploy {
  mode: "snapshot"
}
```

Snapshot mode can:

- Precompile route modules.
- Precompute route manifests.
- Extract scoped CSS.
- Fingerprint assets.
- Reduce cold-start latency.

Snapshot mode must remain optional.

## Serverless

Serverless deployments should support:

- Runtime file loading.
- Lazy compilation.
- Cache reuse between warm invocations.
- KV, D1, Postgres, Redis, or adapter-backed sessions.
- Streaming responses where the platform supports them.

Stateful sessions should store only a signed opaque session ID in the cookie.

## Edge

Edge deployments may prefer stateless tokens:

```web
@auth {
  mode: "stateless-token"
}
```

But stateful sessions are still recommended when revocation matters.

## Streaming

`@defer` requires response streaming:

```web
@defer {
  <SlowPanel />
} @placeholder {
  <Skeleton />
}
```

If the platform does not support streaming, the runtime can:

- Render the placeholder and fetch the deferred region later.
- Wait for all deferred blocks before sending HTML.
- Disable deferral with a clear runtime warning in development.

## Runtime Cache

The runtime cache should key by:

- File path.
- File content hash.
- Compiler version.
- Runtime feature flags.
- Environment mode.

This prevents stale compiled code after source or runtime changes.

## Build Rule

The rule is:

> Build steps are allowed as an optimization, never as a requirement.

The default workflow must remain direct source deployment.

