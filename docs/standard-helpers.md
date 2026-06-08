# Standard Helpers

This page lists common helpers expected from the WebScript runtime.

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

## Password Helpers

```web
password.hash(value)
password.verify(value, hash)
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

