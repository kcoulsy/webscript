# Language Syntax

WebScript uses `.web` files that combine directives, typed server logic, and HTML-like markup.

## File Shape

A page file commonly contains:

```web
@page "/route"

@require auth

@query {
  page: int = 1
}

@load {
  data: Data = await loadData(page)
}

<main>
  <h1>{data.title}</h1>
</main>
```

Directives start with `@`. Markup can appear directly after route, load, action, or config declarations.

## Comments

```web
// Single-line comment

/*
  Multi-line comment
*/
```

Comments are ignored by the parser and not emitted to HTML.

## Variables

Use `@let` inside markup for simple synchronous values:

```web
@let isEmpty: bool = posts.length == 0
```

Inside server logic blocks, use typed declarations:

```web
count: int = 5
posts: Post[] = await Post.recent(count)
```

The `:=` shorthand can infer the type:

```web
post := await Post.find(id)
```

Use explicit types at public boundaries and inferred types for local implementation details.

## Primitive Types

Common primitive types:

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

Durations support readable literals:

```web
15m
1h
30d
```

## Nullable Types

Use `?` for nullable values:

```web
user: User? = await User.find(id)
```

Check for null before accessing fields:

```web
if user == null {
  return notFound()
}

<p>{user.name}</p>
```

## Arrays

```web
users: User[] = await User.all()
```

Iterate arrays with `@for`:

```web
@for user in users {
  <UserCard user={user} />
}
```

## Objects

Object literals use field syntax:

```web
return created({
  id: post.id
  title: post.title
})
```

For function calls with object parameters:

```web
post := await Post.create {
  title: input.title
  slug: slug(input.title)
  authorId: auth.user.id
}
```

## Strings

Use quoted strings:

```web
title: string = "Dashboard"
```

String interpolation is allowed in markup attributes and string expressions:

```web
<a href="/posts/{post.slug}">{post.title}</a>
```

## Blocks

Directive blocks use braces:

```web
@do {
  discount: int = user.isPremium ? 20 : 0
  total: Money = cart.total.minusPercent(discount)
}
```

Variables declared in a block are scoped to that block and its child markup.

## Expressions

Expressions can be used in `{...}`:

```web
<p>Total: {total}</p>
<button disabled={isSubmitting}>Save</button>
```

Expressions should be side-effect free inside markup. Mutations belong in `@action`, `@do`, event handlers, or server logic.

## Directives

Common directives:

```web
@page
@api
@layout
@component
@props
@query
@body
@headers
@load
@action
@require
@auth
@guest
@session
@let
@do
@if
@else
@for
@await
@loading
@then
@error
@defer
@placeholder
@client
@style
@deploy
```

