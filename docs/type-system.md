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

## Response Types

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

## Session Types

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

The compiler should catch:

- Missing component props.
- Wrong prop types.
- Invalid route param usage.
- Nullable values used without checks.
- Invalid response return types.
- Invalid form input shapes.
- Server-only values used in `@client`.

