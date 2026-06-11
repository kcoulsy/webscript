# Auth And Sessions

:::warning[Not Yet Implemented]
Most auth and session features documented on this page are not yet implemented in the current MVP runtime. The current MVP only supports a basic in-memory session with a `webscript_session` cookie. Typed `@session` declarations, `@auth` configuration, `@require auth`/`@guest` guards, `auth.login()`/`auth.logout()`, session rotation, CSRF, and session stores (KV, D1, Postgres, Redis) are not yet available.
:::

Auth is a native request feature in WebScript. It is configured once, loaded for every request, and exposed as typed context.

## Auth Configuration

```web
@auth {
  driver: "session"
  cookie: "__Host_app_session"
  sameSite: "lax"
  secure: true
  httpOnly: true
  ttl: 30d
}
```

The runtime handles cookie reading, signature verification, session loading, expiry, refresh, rotation, and logout invalidation.

## Session Configuration

For serverless, the session cookie should only contain a signed opaque session ID.

```txt
Cookie: __Host_app_session=sess_abc123.signature
```

Session data lives in a server-side store:

```web
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

MVP note: the local Rust dev runtime currently provides a tiny in-memory session store. New sessions get a `webscript_session` HttpOnly cookie and expose a `session` object in markup and actions:

```web
<p>{session.count}</p>

@action increment {
  session.count = session.count + 1
  redirect("/")
}
```

This is intentionally not the final auth system: the cookie is not signed yet, sessions reset when the dev server restarts, and typed `@session` declarations are still future work.

## Stateful Session Mode

```web
@auth {
  mode: "stateful-session"
  store: kv("sessions")
}
```

This should be the default for applications.

Request flow:

1. Read signed session cookie.
2. Verify the signature.
3. Load session data from KV, D1, Postgres, Redis, or another configured store.
4. Load the user.
5. Expose typed `auth`.
6. Rotate or refresh the cookie when needed.

## Stateless Token Mode

```web
@auth {
  mode: "stateless-token"
  cookie: "__Host_app_auth"
}
```

This mode is useful for edge and serverless deployments where avoiding a session store matters. It is harder to revoke because state is carried by the token.

Use stateful sessions unless you have a clear reason not to.

## Require Auth

Short form:

```web
@require auth
```

Block form:

```web
@auth required {
  redirect: "/login"
}
```

Then use typed auth context:

```web
<p>{auth.user.name}</p>
```

## Guest-Only Pages

```web
@guest {
  redirect: "/dashboard"
}
```

Use this for login and registration pages.

## Role Guards

```web
@require role("admin")
```

Multiple roles:

```web
@require anyRole("admin", "support")
```

Permission-style guard:

```web
@require can("posts.publish")
```

## Login

```web
@action login(input: LoginForm) -> Redirect {
  user: User? = await User.findByEmail(input.email)

  if user == null || !crypto.verifyPassword(input.password, user.passwordHash) {
    fail("Invalid email or password")
  }

  await auth.login(user.id)

  redirect("/dashboard")
}
```

`auth.login(...)` should:

- Rotate the session ID.
- Store the user ID in the session.
- Refresh the TTL.
- Set an HttpOnly, Secure cookie.
- Generate or rotate the CSRF token.

## Logout

```web
@action logout() -> Redirect {
  await auth.logout()
  redirect("/login")
}
```

`auth.logout()` should:

- Delete the server-side session.
- Expire the cookie.
- Clear auth context for the current response.

## Conditional UI

```web
@if auth.check {
  <p>Signed in as {auth.user.email}</p>

  <form @submit={logout}>
    <button>Logout</button>
  </form>
} @else {
  <a href="/login">Login</a>
}
```

## Auth Context

Recommended shape:

```web
auth.check: bool
auth.user: User
auth.userId: UserId?
auth.roles: string[]
auth.session: Session
```

`auth.user` is only non-null when `auth.check` is true or the route requires auth.

## Native Runtime Responsibilities

The language/runtime handles:

- `Set-Cookie`
- HttpOnly cookies
- Secure cookies
- SameSite policy
- Signed session IDs
- CSRF tokens
- Session rotation on login
- Expiry
- TTL refresh
- Logout invalidation
- Role and permission guards
