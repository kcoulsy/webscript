# Routing

WebScript supports explicit route declarations for pages and API endpoints.

:::warning[Not Yet Implemented]
Several routing features are documented but not yet implemented in the current MVP runtime:
- `@api` routes (only `@page` routes are available)
- `@query` for typed query parameters
- Route guards (`@require auth`, `@require role(...)`, `@require anyRole(...)`, `@require can(...)`)
- `@auth required { redirect }` and `@guest { redirect }`
- `@headers` for response headers
- Custom scalar types in route params (e.g., `{id:UserId}`)
:::

## Pages

Pages render HTML by default:

```web
@page "/"

<h1>Home</h1>
```

## Dynamic Segments

```web
@page "/posts/{slug:string}"

@load {
  post: Post? = await Post.findBySlug(slug)

  if post == null {
    return notFound()
  }
}

<h1>{post.title}</h1>
```

Route params are typed from the route pattern.

## Common Param Types

```web
{id:int}
{slug:string}
{userId:UserId}
{uuid:uuid}
```

Custom scalar types can validate and parse params.

## Query Parameters

Use `@query` to define accepted query parameters:

```web
@query {
  page: int = 1
  search: string? = null
  tags: string[] = []
}
```

The runtime parses, validates, and exposes these values as typed variables.

Invalid query values should produce a `400 Bad Request` response unless a page overrides that behavior.

## Guards

Require auth:

```web
@require auth
```

Require a role:

```web
@require role("admin")
```

Full auth block:

```web
@auth required {
  redirect: "/login"
}
```

Guest-only page:

```web
@guest {
  redirect: "/dashboard"
}
```

## Route Methods

Pages usually respond to `GET`. Actions handle mutations from forms.

API routes declare methods:

```web
@api GET "/api/posts"
@api POST "/api/posts"
@api DELETE "/api/posts/{id:int}"
```

## Headers On Pages

```web
@headers {
  "Cache-Control": "no-store"
  "X-Frame-Options": "DENY"
}
```

Headers apply to the response unless a returned response overrides them.

## Not Found

Return `notFound()` from route logic:

```web
if post == null {
  return notFound()
}
```

For pages, `notFound()` can render a configured 404 page. For APIs, it returns a typed error body if provided.

## Redirects

```web
return redirect("/dashboard")
```

Permanent redirect:

```web
return redirect("/new-url", status: 301)
```

## Route Precedence

Recommended precedence:

1. Exact static routes.
2. Typed dynamic routes.
3. Catch-all routes.
4. File-based fallback routes.

Explicit `@page` and `@api` declarations should be preferred over inferred routes when both exist.

