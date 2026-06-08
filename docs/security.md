# Security

WebScript security should be default-on. Common web security features belong in the runtime, not in application boilerplate.

## HTML Escaping

Dynamic values are escaped by default:

```web
<p>{user.name}</p>
```

Use trusted HTML only for sanitized content:

```web
<article>{html.trusted(post.renderedBody)}</article>
```

## Cookies

Auth cookies should default to:

```web
httpOnly: true
secure: true
sameSite: "lax"
```

Session cookies should use the `__Host_` prefix when possible:

```web
cookie: "__Host_app_session"
```

## Session Contents

Stateful session cookies should store only a signed opaque session ID:

```txt
sess_abc123.signature
```

Do not store the whole user object in the cookie.

## CSRF

Session-authenticated unsafe methods require CSRF protection:

- `POST`
- `PUT`
- `PATCH`
- `DELETE`

The runtime should generate, store, verify, and rotate CSRF tokens.

## Session Rotation

On login:

- Rotate the session ID.
- Refresh the TTL.
- Regenerate CSRF token.
- Set a fresh cookie.

On logout:

- Delete server-side session data.
- Expire the cookie.
- Prevent old session reuse.

## Auth Guards

Use route-level guards:

```web
@require auth
@require role("admin")
```

For pages, missing auth can redirect. For APIs, missing auth should return `401`.

## Headers

Recommended secure defaults:

```web
@headers {
  "X-Frame-Options": "DENY"
  "X-Content-Type-Options": "nosniff"
  "Referrer-Policy": "strict-origin-when-cross-origin"
}
```

Content Security Policy should be configurable:

```web
@headers {
  "Content-Security-Policy": "default-src 'self'"
}
```

## Secrets

Secrets should come from environment bindings:

```web
secret := env("SESSION_SECRET")
```

Never expose secrets to `@client`, serialized props, logs, or error pages.

## Server And Client Boundary

These values are server-only:

- Database connections.
- Session stores.
- Secret keys.
- Raw request headers.
- Signed cookies.
- `auth.session`.

Only serializable, non-sensitive data should be passed into client islands.

## Passwords

Use runtime password helpers:

```web
password.hash(input.password)
password.verify(input.password, user.passwordHash)
```

The helper should use a modern password hashing algorithm such as Argon2id or bcrypt with safe defaults.

## Rate Limiting

Recommended native helper:

```web
@action login(input: LoginForm) -> Redirect {
  await rateLimit("login", key: request.ip, max: 5, window: 10m)
  // ...
}
```

Login, password reset, registration, and webhook endpoints should support rate limiting.

