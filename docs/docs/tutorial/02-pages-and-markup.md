---
sidebar_position: 2
title: Pages and markup
---

# Pages and markup

WebScript pages combine directives (lines starting with `@`) and HTML-like markup. Values are embedded with `{expression}`.

## Variables with `@let`

Add local values above your markup:

```web
@page "/"

@let name = "WebScript"
@let visits = 2 + 3
@let greeting = "Hello " + name
@let author = {
  name: "Ada"
  role: "admin"
}
@let posts = [
  { title: "Intro", slug: "intro", featured: true },
  { title: "Launch", slug: "launch", featured: false }
]

<main>
  <h1>{greeting}</h1>
  <p>Author: {author.name} ({author.role})</p>
  <p>Visit count: {visits + 1}</p>
</main>
```

Types are inferred for primitives, objects, and arrays. Use explicit types at public boundaries; infer locally when the value is obvious.

## Expressions in markup

Use `{...}` for dynamic text and attributes:

```web
<a href="/posts/{post.slug}">{post.title}</a>
<button disabled={isSubmitting}>Save</button>
```

Supported operators include `+`, `-`, comparisons, `&&`, `||`, and `!`.

## Control flow

### `@if` / `@else`

```web
@let ready = visits == 5

@if ready {
  <p>The page is ready.</p>
} @else {
  <p>Still loading context.</p>
}
```

### `@for`

```web
@for post in posts {
  <article>
    <h2>{post.title}</h2>
  </article>
}
```

Loop variables are scoped to the loop body.

## Sync server blocks with `@do`

Use `@do` for short synchronous calculations that should not live in the template:

```web
@do {
  discount: int = 0
  if visits > 4 {
    discount = 10
  }
  promo: string = "Save " + discount + "%"
}

<p>{promo}</p>
```

`@do` cannot use `await`, `fetch`, `spawn`, `timeout`, or `throw`. Async work belongs in `@load` or `@action`.

## Comments

```web
// single-line

/*
  multi-line
*/
```

Next you will extract repeated markup into a component.
