---
sidebar_position: 5
title: Directives
---

# Directive Reference

Directives start with `@` and define routes, data, markup logic, runtime behavior, and configuration.

## Route Directives

### `@page`

Declares an HTML page route.

```web
@page "/dashboard"
```

### `@api`

Declares an API route.

```web
@api GET "/api/posts" -> Json<Post[]>
```

### `@layout`

Defines or applies a layout.

```web
@layout AppLayout { title: "Dashboard" }
```

## Data Directives

### `@query`

Parses query parameters.

```web
@query {
  page: int = 1
}
```

### `@body`

Parses request body data.

```web
@body input: CreatePost
```

### `@load`

Loads server data before rendering.

```web
@load {
  posts: Post[] = await Post.published()
}
```

### `@action`

Defines a server mutation.

```web
@action save(input: SaveInput) -> Redirect {
  redirect("/done")
}
```

## Markup Logic Directives

### `@let`

Declares one synchronous local value.

```web
@let isEmpty: bool = posts.length == 0
```

### `@do`

Runs multiple synchronous statements.

```web
@do {
  total: Money = cart.total
}
```

### `@if` / `@else`

Conditionally renders markup.

```web
@if auth.check {
  <AccountMenu />
} @else {
  <LoginLink />
}
```

### `@for`

Loops over arrays.

```web
@for post in posts {
  <PostCard post={post} />
}
```

## Async Directives

### `@await`

Renders async data with optional loading and error states.

```web
@await db.users.recent() as users {
  <UserList users={users} />
} @loading {
  <Spinner />
} @error err {
  <ErrorBox message={err.message} />
}
```

### `@async`

Verbose async state block.

```web
@async users: User[] = db.users.recent() {
  @then users {
    <UserList users={users} />
  }
}
```

### `@defer`

Streams a lazy section.

```web
@defer {
  <SlowPanel />
} @placeholder {
  <Skeleton />
}
```

## Auth Directives

### `@auth`

Configures auth or requires auth for a route.

```web
@auth required {
  redirect: "/login"
}
```

### `@require`

Runs a route guard.

```web
@require auth
@require role("admin")
```

### `@guest`

Restricts a route to signed-out users.

```web
@guest {
  redirect: "/dashboard"
}
```

### `@session`

Defines session store and typed session data.

```web
@session {
  data {
    userId: UserId
  }
}
```

## Response Directives

### `@headers`

Sets response headers.

```web
@headers {
  "Cache-Control": "no-store"
}
```

## Client Directives

### `@client`

Declares browser-only state.

```web
@client {
  count: signal<int> = 0
}
```

## Styling Directives

### `@style`

Defines component or page styles. Place `@style` blocks **after markup** in the file.

```web
@style {
  .card { padding: 1rem; }
}

@style global {
  body { margin: 0; }
}
```

- `@style { }` — scoped to markup from that file (default)
- `@style scoped { }` — same as `@style { }`
- `@style global { }` — appended to the page as global CSS

## Runtime Directives

### `@deploy`

Configures deployment mode or adapter.

```web
@deploy {
  mode: "runtime"
}
```

### `@features`

Enables runtime features.

```web
@features {
  streaming: true
}
```

