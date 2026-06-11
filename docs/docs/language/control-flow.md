---
sidebar_position: 2
---

# Control Flow

WebScript supports control flow in server logic and directly inside markup.

## `@if`

```web
@if auth.check {
  <p>Signed in as {auth.user.email}</p>
} @else {
  <a href="/login">Login</a>
}
```

The condition must be a `bool`.

## `@else if`

```web
@if status == "draft" {
  <Badge>Draft</Badge>
} @else if status == "published" {
  <Badge>Published</Badge>
} @else {
  <Badge>Archived</Badge>
}
```

## `@for`

```web
@for user in users {
  <UserCard user={user} />
}
```

The loop variable is scoped to the loop body.

## Indexes

:::warning[Not Yet Implemented]
The `@for item, index in items` syntax with an index variable is documented but not yet implemented in the current MVP runtime.
:::

```web
@for user, index in users {
  <p>{index + 1}. {user.name}</p>
}
```

Indexes are zero-based.

## Empty State

:::warning[Not Yet Implemented]
The `@empty` branch on `@for` loops is documented but not yet implemented in the current MVP runtime.
:::

Recommended syntax:

```web
@for post in posts {
  <PostCard post={post} />
} @empty {
  <EmptyState />
}
```

Use `@empty` instead of precomputing emptiness when the empty branch only belongs to a loop.

## `@let`

Use `@let` for a single synchronous local value:

```web
@let showAdmin: bool = user.role == "admin"

@if showAdmin {
  <AdminLinks />
}
```

`@let` should not await promises. Use `@await`, `@load`, or `@defer` for async values.

## `@do`

Use `@do` for synchronous server statements inside markup:

```web
@do {
  discount: int = user.isPremium ? 20 : 0
  total: int = price - discount
}

<p>Total: {total}</p>
```

`@do` rejects `await`, `fetch`, `spawn`, `timeout`, and `throw`. See [Server Logic](./server-logic).

## `@do`

Use `@do` for multiple synchronous statements inside markup:

```web
@do {
  discount: int = user.isPremium ? 20 : 0
  total: Money = cart.total.minusPercent(discount)
}

<p>Total: {total}</p>
```

`@do` is useful when the template needs local derived values but the logic is not important enough to move into `@load`.

## Server Logic `if`

Inside `@load`, `@action`, `@api`, or `@defer`, use normal logic:

```web
@load {
  post := await Post.find(slug)

  if post == null {
    return notFound()
  }
}
```

Returning a response from `@load` stops normal page rendering.

## Scope Rules

Variables are visible in their lexical scope:

```web
@if auth.check {
  @let email: string = auth.user.email
  <p>{email}</p>
}

// email is not available here
```

Parent scope values are visible to child scopes:

```web
@let limit: int = 5

@await db.orders.recent(limit) as orders {
  <OrderSkeleton count={limit} />
}
```

