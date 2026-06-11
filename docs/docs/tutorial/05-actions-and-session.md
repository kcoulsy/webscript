---
sidebar_position: 5
title: Actions and session
---

# Actions and session

`@action` blocks handle server-side mutations: form posts, session updates, redirects, and validation errors.

## A simple counter

On the home page, increment a session value:

```web
@page "/"

@action increment {
  session.count = session.count + 1
  redirect("/")
}

<main>
  <p>Session count: {session.count}</p>

  <form method="post" action="/">
    <input type="hidden" name="_action" value="increment" />
    <button>Increment</button>
  </form>
</main>
```

The hidden `_action` field selects which `@action` runs. Actions execute on the server and can redirect when done.

## Validation with `fail`

```web
@action rememberName {
  if input.name == "" {
    fail("Name is required")
  }
  session.name = input.name
  redirect("/")
}

<form method="post" action="/">
  <input type="hidden" name="_action" value="rememberName" />
  <input name="name" value={session.name} />
  <button>Remember name</button>
</form>
```

`fail(...)` returns a validation error to the client instead of redirecting.

## Loops in actions

Server blocks support `fn`, `while`, and `try/catch`:

```web
@action validateWithWhile {
  attempts := 0
  while attempts < 3 && input.name == "" {
    attempts = attempts + 1
  }
  if input.name == "" {
    fail("Name still required after " + attempts + " checks")
  }
  session.name = input.name
  redirect("/")
}
```

## Typed form input

:::warning[Not Yet Implemented]
`@body input: TypeName` and typed action inputs (e.g., `@action login(input: LoginForm) -> Redirect`) with `auth.login()` are documented but not yet implemented in the current MVP runtime. See [Forms and Validation](../guides/forms-and-validation) for the current MVP form pattern.
:::

For structured forms, declare `@body`:

```web
@body input: LoginForm

@action login(input: LoginForm) -> Redirect {
  user: User? = await User.findByEmail(input.email)
  if user == null {
    fail("Invalid email or password")
  }
  await auth.login(user.id)
  redirect("/dashboard")
}
```

See [Forms and Validation](../guides/forms-and-validation) and [Auth and Sessions](../guides/auth-and-sessions) for login flows and guards.

Next you will use async builtins in `@load`.
