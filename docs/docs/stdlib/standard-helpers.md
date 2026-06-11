---
sidebar_position: 1
---

# Standard Helpers

This page lists common helpers expected from the WebScript runtime.

:::warning[Not Yet Implemented]
Most helpers documented on this page are not yet implemented in the current MVP runtime. The currently available helpers are limited to:
- `crypto.hashPassword(value)` and `crypto.verifyPassword(value, hash)` (Argon2id)
- `await fetch(url, Schema)` (HTTP GET)
- `await sleep(duration)`, `spawn(promise)`, `timeout(duration, promise)`
- `fail("message")` and `redirect("/path")` in actions
- Basic `session` access in actions and templates
- Debug bar at `/.web/debug`

The following are documented but not yet available:
- Response helpers (`response()`, `html()`, `json()`, `ok()`, `created()`, `noContent()`, `badRequest()`, `unauthorized()`, `forbidden()`, `notFound()`, `conflict()`, `unprocessable()`, `serverError()`, `redirect(path, status)`)
- Response modifiers (`.status()`, `.header()`, `.cookie()`, `.signedCookie()`, `.forgetCookie()`)
- Auth helpers (`auth.check`, `auth.user`, `auth.userId`, `auth.roles`, `auth.session`, `auth.login()`, `auth.logout()`)
- Guard helpers (`role()`, `anyRole()`, `can()`, `require()`)
- Request helpers (`request.header()`, `request.cookie()`, `request.accepts()`, `request.ip`, `request.userAgent`)
- String/HTML helpers (`slug()`, `escape()`, `html.trusted()`, `asset()`, `formatDate()`)
- Logging helpers (`log.debug()`, `log.info()`, `log.warn()`, `log.error()`)
- Rate limiting (`rateLimit()`)
- Storage helpers (`kv()`, `storage.put()`, `storage.get()`)
- Database helpers (`db.transaction { ... }`, `Model.findByEmail()`)
- Environment (`env()`)
:::

## Response Helpers

```web
response()
html(markup)
json(data)
ok(data)
created(data)
noContent()
badRequest(error)
unauthorized(error)
forbidden(error)
notFound(error)
conflict(error)
unprocessable(error)
serverError(error)
redirect(path, status: 303)
```

## Response Modifiers

```web
.status(code)
.header(name, value)
.cookie(name, value, options)
.signedCookie(name, value, options)
.forgetCookie(name)
```

## Auth Helpers

```web
auth.check
auth.user
auth.userId
auth.roles
auth.session
auth.login(userId)
auth.logout()
```

## Guard Helpers

```web
role("admin")
anyRole("admin", "support")
can("posts.publish")
require(condition)
```

## Request Helpers

```web
request.header("Authorization")
request.cookie("theme")
request.accepts("application/json")
request.ip
request.userAgent
```

## Crypto Helpers

```web
crypto.hashPassword(value)
crypto.verifyPassword(value, hash)
```

## String Helpers

```web
slug("Hello World")
escape(value)
```

## HTML Helpers

```web
html.trusted(value)
asset("images/logo.png")
```

## Date Helpers

```web
now()
formatDate(date)
```

## Logging

```web
log.debug(message, context)
log.info(message, context)
log.warn(message, context)
log.error(error, context)
```

## Rate Limiting

```web
rateLimit("login", key: request.ip, max: 5, window: 10m)
```

## Storage

```web
kv("sessions")
storage.put(file)
storage.get(path)
```

## Database

Database helpers are adapter-dependent, but the language examples assume async model methods:

```web
User.find(id)
User.findByEmail(email)
Post.published()
Post.create(input)
db.transaction { ... }
```

