# Async Rendering

WebScript makes async blocks first-class inside HTML. This lets a page render useful structure while slow data loads, without moving template logic into a separate framework layer.

## `@await`

Use `@await` when markup depends on a promise:

```web
@await db.users.recent() as users {
  <UserList users={users} />
} @loading {
  <Spinner />
} @error err {
  <p>Could not load users: {err.message}</p>
}
```

The `@await` block has three states:

- Main block: renders when the promise resolves.
- `@loading`: renders while the promise is pending.
- `@error`: renders if the promise rejects.

## Typed Async Declaration

Use `@async` when you want to name and type the async value explicitly:

```web
@async users: User[] = db.users.recent() {
  @loading {
    <Spinner />
  }

  @error err {
    <ErrorBox message={err.message} />
  }

  @then users {
    @for user in users {
      <UserCard user={user} />
    }
  }
}
```

This form is verbose but precise. It is useful when the promise type is not obvious or the block needs clearly separated states.

## Mid-HTML Async Logic

Async blocks can appear inside ordinary markup:

```web
<section>
  <h2>Recent orders</h2>

  @let limit: int = 5
  @let ordersPromise: Promise<Order[]> = db.orders.recent(limit)

  @await ordersPromise as orders {
    @for order in orders {
      <OrderRow order={order} />
    }
  } @loading {
    <OrderSkeleton count={limit} />
  }
</section>
```

## `@defer`

Use `@defer` for lazy streamed sections:

```web
@defer {
  <SlowStatsPanel />
} @placeholder {
  <StatsSkeleton />
}
```

The intended server behavior is:

1. Send the page shell immediately.
2. Render the placeholder first.
3. Resolve the deferred block.
4. Stream replacement HTML.
5. Let the optional client runtime swap the placeholder with the final HTML.

## Deferred Data

```web
<body>
  <Hero />

  @defer {
    stats: Stats = await analytics.getStats()

    <StatsCard stats={stats} />
  } @placeholder {
    <StatsCardSkeleton />
  }

  <Footer />
</body>
```

The deferred block can contain server statements followed by markup.

## Deferred Error State

```web
@defer {
  posts: Post[] = await db.posts.byUser(user.id)

  <section>
    <h2>Posts</h2>

    @for post in posts {
      <PostCard post={post} />
    }
  </section>
} @placeholder {
  <PostSkeleton count={3} />
} @error err {
  <ErrorBox message="Could not load posts" />
}
```

The `@error` branch should be safe to render after the initial shell has already been sent.

## `@await` Versus `@defer`

Use `@await` when:

- The server can wait before sending the response.
- The block should render as part of the normal response.
- You need loading and error states for client-side navigation or progressive rendering.

Use `@defer` when:

- The page shell should be sent before slow data resolves.
- The block is non-critical.
- A placeholder is acceptable.
- Streaming replacement HTML is available.

## Streaming IDs

Each deferred block should receive a stable runtime ID:

```html
<template data-web-defer="stats-panel"></template>
```

The runtime can use that ID to replace the placeholder when the streamed chunk arrives.

Developers should rarely need to manage these IDs manually.

## Cancellation

If the client disconnects, the runtime should cancel pending deferred work where possible. Database drivers and fetch calls should receive an abort signal through the request context.

