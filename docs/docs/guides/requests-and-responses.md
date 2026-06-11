# Requests And Responses

:::warning[Not Yet Implemented]
Most request/response helpers documented on this page are not yet implemented in the current MVP runtime. This includes the `request` context object, response helpers, chainable modifiers, `.cookie()`, `.signedCookie()`, `.forgetCookie()`, and `@headers`. See [Data Loading and Actions](./data-loading-and-actions) for what the current MVP supports.
:::

Responses are native values in WebScript. Pages return HTML by default, API routes return JSON by default, and actions can return redirects, JSON, HTML, or errors.

## Request Context

Every route has access to `request`:

```web
request.method
request.url
request.path
request.params
request.query
request.headers
request.cookies
request.body
request.ip
request.userAgent
```

## Headers

Read headers:

```web
token: string? = request.header("Authorization")
```

Set route headers:

```web
@headers {
  "Cache-Control": "public, max-age=60"
  "X-Frame-Options": "DENY"
}
```

Set response headers:

```web
return json(posts)
  .header("Cache-Control", "public, max-age=60")
  .header("X-Source", "webscript")
```

## Cookies

Read cookies:

```web
theme: string = request.cookie("theme") ?? "light"
```

Set cookies:

```web
return redirect("/dashboard")
  .cookie("theme", "dark", {
    httpOnly: false
    secure: true
    sameSite: "lax"
    maxAge: 30d
  })
```

Delete cookies:

```web
return redirect("/")
  .forgetCookie("theme")
```

Signed cookies:

```web
return response()
  .signedCookie("preview_token", token, {
    httpOnly: true
    secure: true
    sameSite: "strict"
    maxAge: 15m
  })
```

## JSON

```web
return json(posts)
```

With status:

```web
return json({ error: "Not found" }, status: 404)
```

Typed response:

```web
@api GET "/api/posts" -> Json<Post[]>
```

## Redirects

```web
return redirect("/dashboard")
```

With status:

```web
return redirect("/posts", status: 303)
```

## Status Helpers

```web
ok(data)              // 200
created(data)         // 201
noContent()           // 204
badRequest(error)     // 400
unauthorized(error)   // 401
forbidden(error)      // 403
notFound(error)       // 404
conflict(error)       // 409
unprocessable(error)  // 422
serverError(error)    // 500
```

## Chainable Modifiers

Responses can be modified fluently:

```web
return created(post)
  .header("Location", "/api/posts/{post.id}")
  .cookie("last_post_id", post.id, {
    httpOnly: true
    secure: true
    sameSite: "lax"
    maxAge: 1d
  })
```

## Pages Returning Responses

HTML pages can return non-HTML responses from `@load`:

```web
@page "/preview/{token:string}"

@headers {
  "Cache-Control": "no-store"
}

@load {
  preview := await Preview.find(token)

  if preview == null {
    return notFound()
  }
}

<h1>{preview.title}</h1>
```

## Response Defaults

Recommended defaults:

- Pages return HTML.
- API routes return JSON.
- Actions return whatever their declared return type says.
- Validation failures return `422`.
- Missing resources return `404`.
- Auth failures return redirect for pages and `401` for APIs.
- Permission failures return `403`.

