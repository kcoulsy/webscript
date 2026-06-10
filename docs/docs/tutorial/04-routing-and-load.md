---
sidebar_position: 4
title: Routing and data loading
---

# Routing and data loading

## Dynamic routes

Add `app/pages/posts.web` with a typed path parameter:

```web
@page "/posts/{slug:string}"

@let title = "Post: " + slug
@let isIntro = slug == "intro"

<main>
  <h1>{title}</h1>

  @if isIntro {
    <p>This is the featured intro post.</p>
  } @else {
    <p>Slug: {slug}</p>
  }

  <nav><a href="/">Home</a></nav>
</main>
```

Visit `/posts/intro` and `/posts/launch`. The `slug` param is parsed from the URL and available as a typed variable.

Common param types: `{id:int}`, `{slug:string}`, `{uuid:uuid}`.

## `@load` — server data before render

`@load` runs on the server before the page template. Use it for async fetches, validation, and early returns.

```web
@page "/posts/{slug:string}"

@load {
  _: object = await sleep(5ms)
  headline: string = "Loaded post: " + slug
}

<main>
  <h1>Post: {slug}</h1>
  <p>{headline}</p>
</main>
```

Values declared in `@load` are available in the template. `sleep` is useful for simulating latency during development.

## Returning responses from `@load`

Stop rendering and return a response when data is missing:

```web
@load {
  post := await Post.findBySlug(slug)

  if post == null {
    return notFound()
  }
}
```

## Query parameters

Accept typed query strings with `@query`:

```web
@page "/posts"

@query {
  page: int = 1
  search: string? = null
}
```

Invalid query values produce `400 Bad Request` by default.

See [Routing](../guides/routing) and [Data Loading and Actions](../guides/data-loading-and-actions) for guards, layouts, and API routes.

Next you will handle form submissions with `@action`.
