---
sidebar_position: 3
---

# Type System

WebScript is typed at route boundaries, data loading boundaries, action inputs, component props, auth/session context, and response values.

## Primitive Types

```web
string
int
float
bool
date
datetime
duration
bytes
```

:::warning[Not Yet Implemented]
The `date`, `datetime`, and `bytes` primitive types are documented but not yet implemented in the current MVP runtime. Only `string`, `int`, `float`, `bool`, and `duration` are currently supported.
:::

## String Literal Types

Use string literal types to restrict a value to one allowed string, or a small inline union of strings:

```web
@component Button {
  variant: "primary" | "secondary" = "primary"
  type: "button" | "submit" | "reset" = "button"
}
```

Only double-quoted string literals are supported in type positions. Named aliases for literal unions are not part of this version.

## Nullable Types

```web
user: User?
```

Nullable values must be checked before use:

```web
if user == null {
  return notFound()
}

<p>{user.name}</p>
```

## Arrays

```web
posts: Post[]
```

## Promises

:::warning[Not Yet Implemented]
The `Promise<T>` type annotation is documented but not yet available in the current MVP runtime.
:::

```web
ordersPromise: Promise<Order[]> = db.orders.recent(limit)
```

Use `await` in server logic or `@await` in markup.

## Object Types

```web
type User {
  id: UserId
  name: string
  email: string
}
```

## Input Types

```web
type CreatePost {
  title: string
  content: string
}
```

Input types can include validation annotations:

```web
type RegisterInput {
  email: string @email
  password: string @min(12)
}
```

:::warning[Not Yet Implemented]
Validation annotations on input types (`@email`, `@min`, `@max`) inside `type` declarations are documented but not yet implemented. Use `@schema` files for runtime validation instead.
:::

## Response Types

:::warning[Not Yet Implemented]
Response types (`Json<T>`, `Redirect`, `Html`, `Response`) in action and API declarations are documented but not yet implemented in the current MVP runtime.
:::

```web
Json<Post[]>
Redirect
Html
Response
```

Example:

```web
@api GET "/api/posts" -> Json<Post[]>
```

## Component Prop Types

```web
@component UserCard {
  user: User
  compact: bool = false
}
```

Props can also use string literal unions:

```web
@component Badge {
  tone: "info" | "success" | "warning" = "info"
}
```

## Session Types

:::warning[Not Yet Implemented]
Typed `@session` declarations are documented but not yet implemented. The current MVP uses a basic in-memory session with string keys. See [Auth and Sessions](../guides/auth-and-sessions) for current limitations.
:::

```web
@session {
  data {
    userId: UserId
    roles: string[]
    csrfToken: string
  }
}
```

The runtime uses this declaration to type `auth.session`.

## Type Inference

Use `:=` for local inference:

```web
post := await Post.find(id)
```

Prefer explicit types for:

- Public APIs.
- Action inputs.
- Component props.
- Session data.
- Auth context.
- Route params.

Use inference for:

- Local variables.
- Short-lived derived values.
- Intermediate query results.

## Custom Scalar Types

:::warning[Not Yet Implemented]
Custom scalar types are documented but not yet implemented in the current MVP runtime.
:::

Custom scalar types can parse route params and input fields:

```web
type UserId scalar string
type Slug scalar string
```

Route usage:

```web
@page "/users/{id:UserId}"
```

## Type Errors

:::warning[Not Yet Implemented]
Full type-checking (missing props, wrong types, nullable access, server/client boundary enforcement) is documented but not yet enforced in the current MVP runtime.
:::

The compiler should catch:

- Missing component props.
- Wrong prop types.
- Invalid route param usage.
- Nullable values used without checks.
- Invalid response return types.
- Invalid form input shapes.
- Server-only values used in `@client`.
