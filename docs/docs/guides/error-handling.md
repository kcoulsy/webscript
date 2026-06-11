# Error Handling

:::warning[Not Yet Implemented]
Several error handling features documented on this page are not yet implemented in the current MVP runtime:
- Response helpers (`notFound()`, `forbidden()`, `badRequest()`, `unprocessable()`, `serverError()`)
- Structured field-level `fail({ field: "message" })` — only `fail("message")` is currently available
- `@error err { }` error boundaries on pages and components
- Custom error pages (`@page error 404`, `@page error 500`, `app/pages/errors/*.web`)
- `log.debug/info/warn/error` logging helpers
:::

WebScript distinguishes expected application failures from unexpected runtime errors.

## Expected Failures

Use response helpers for expected outcomes:

```web
return notFound({ error: "Post not found" })
return forbidden({ error: "Admins only" })
return badRequest({ error: "Invalid page" })
```

Use `fail(...)` for validation or action failures:

```web
fail("Invalid email or password")
```

## Validation Errors

Field-specific validation:

```web
fail({
  email: "Email is already in use"
})
```

Default status should be `422`.

## Async Errors

`@await` supports error UI:

```web
@await db.users.recent() as users {
  <UserList users={users} />
} @error err {
  <ErrorBox message={err.message} />
}
```

`@defer` supports streamed error fallback:

```web
@defer {
  <SlowPanel />
} @placeholder {
  <Skeleton />
} @error err {
  <ErrorBox message="Could not load panel" />
}
```

## Page Error Boundaries

Pages can define an error boundary:

```web
@error err {
  <main>
    <h1>Something went wrong</h1>
    <p>{err.message}</p>
  </main>
}
```

In production, avoid exposing internal error messages.

## Component Error Boundaries

Components can handle their own render errors:

```web
@component StatsPanel {}

@error err {
  <StatsPanelFallback />
}
```

## Global Error Pages

Recommended files:

```txt
app/pages/errors/404.web
app/pages/errors/500.web
```

404:

```web
@page error 404

<h1>Page not found</h1>
```

500:

```web
@page error 500

<h1>Something went wrong</h1>
```

## Logging

Unexpected errors should be logged with request context:

```web
log.error(err, {
  path: request.path
  userId: auth.userId
})
```

Do not log secrets, passwords, raw session cookies, CSRF tokens, or full authorization headers.

## Production Behavior

In production:

- Hide stack traces from users.
- Return stable error shapes for APIs.
- Render configured error pages for HTML.
- Log internal details server-side.

In development:

- Show source location.
- Show stack trace.
- Show route, params, query, and relevant type errors.
- Link to the source `.web` file when possible.

