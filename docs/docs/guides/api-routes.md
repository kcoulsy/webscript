# API Routes

:::warning[Not Yet Implemented]
API routes (`@api`) and related features documented on this page are not yet implemented in the current MVP runtime. This includes:
- `@api METHOD "/path"` route declarations
- `@query` for typed query parameters
- `@body` for request body parsing
- `@require auth` guards
- JSON response helpers (`ok()`, `created()`, `json()`, `notFound()`, etc.)
- Chainable response modifiers (`.header()`, `.cookie()`, etc.)
- `slug()` string helper

Use `@page` routes with `@load` and `@action` for the current MVP.
:::

API routes are declared with `@api` and are response-first. They usually return JSON.

## Basic API Route

```web
@api GET "/api/posts"

@query {
  page: int = 1
}

@load {
  posts: Post[] = await Post.published().paginate(page)
}

return json(posts)
```

## Explicit Response Type

```web
@api GET "/api/posts" -> Json<Post[]> {
  posts: Post[] = await Post.published()
  return json(posts)
}
```

Explicit types are useful for public APIs and generated API clients.

## Request Body

```web
@api POST "/api/posts" -> Json<Post>

@require auth

@body input: CreatePost

@action {
  post := await Post.create {
    title: input.title
    slug: slug(input.title)
    content: input.content
    authorId: auth.user.id
  }

  return created(post)
}
```

`@body` parses JSON by default for API routes and form data by default for page actions.

## Status Codes

```web
return json({ error: "Post not found" }, status: 404)
```

Preferred helper:

```web
return notFound({
  error: "Post not found"
})
```

## Headers

```web
return ok(posts)
  .header("Cache-Control", "public, max-age=60")
```

## Full Example

```web
@api POST "/api/posts" -> Json<Post>

@require auth

@body input: CreatePost

@action {
  post := await Post.create {
    title: input.title
    slug: slug(input.title)
    content: input.content
    authorId: auth.user.id
  }

  return created(post)
    .header("Location", "/api/posts/{post.id}")
    .cookie("last_post_id", post.id, {
      httpOnly: true
      secure: true
      sameSite: "lax"
      maxAge: 1d
    })
}
```

## Error Shape

Recommended JSON error format:

```json
{
  "error": {
    "code": "post_not_found",
    "message": "Post not found"
  }
}
```

Validation error format:

```json
{
  "error": {
    "code": "validation_failed",
    "message": "Validation failed",
    "fields": {
      "title": "Title is required"
    }
  }
}
```

## Auth Behavior

For API routes:

- Missing auth returns `401`.
- Failed role/permission checks return `403`.
- Auth redirects should not be used unless explicitly configured.

## Caching

```web
@headers {
  "Cache-Control": "public, max-age=60, stale-while-revalidate=300"
}
```

Or dynamic:

```web
return ok(posts)
  .header("Cache-Control", auth.check ? "private, no-store" : "public, max-age=60")
```

