---
sidebar_position: 4
---

# HTML Templates

WebScript templates are HTML-first. Markup is written directly in `.web` files and can contain components, expressions, directives, and scoped logic.

## Basic Markup

```web
<main>
  <h1>Dashboard</h1>
  <p>Welcome back.</p>
</main>
```

HTML is escaped by default when rendering dynamic values:

```web
<p>{user.name}</p>
```

If `user.name` contains HTML, it renders as text unless explicitly marked safe by a trusted helper.

## Expressions

Use `{...}` to insert expressions:

```web
<h1>{post.title}</h1>
<time datetime={post.publishedAt}>{formatDate(post.publishedAt)}</time>
```

Expressions are valid in text nodes, attributes, and component props.

## Boolean Attributes

Boolean attributes render when true and are omitted when false:

```web
<button disabled={form.pending}>Save</button>
```

## Dynamic Classes

Classes can be plain strings:

```web
<div class="card selected"></div>
```

They can also be expressions:

```web
<div class={isActive ? "card active" : "card"}></div>
```

Recommended object form:

:::warning[Not Yet Implemented]
The object class syntax `class={{ "card": true, "active": isActive }}` is documented but not yet implemented in the current MVP runtime.
:::

```web
<div class={{
  "card": true
  "active": isActive
  "muted": disabled
}}></div>
```

## Components In Markup

Components use capitalized tag names:

```web
<UserCard user={user} />
```

Children are passed through slots:

```web
<Panel title="Account">
  <AccountSummary user={user} />
</Panel>
```

## Logic In Markup

Simple local values:

```web
@let isEmpty: bool = posts.length == 0
```

Multiple sync statements:

```web
@do {
  discount: int = user.isPremium ? 20 : 0
  total: Money = cart.total.minusPercent(discount)
}

<p>Total: {total}</p>
```

Conditional rendering:

```web
@if isEmpty {
  <EmptyState />
} @else {
  <PostList posts={posts} />
}
```

Loops:

```web
@for post in posts {
  <PostCard post={post} />
}
```

## Raw HTML

:::warning[Not Yet Implemented]
`html.trusted()` is documented but not yet implemented in the current MVP runtime.
:::

Raw HTML should require an explicit trusted wrapper:

```web
<article>{html.trusted(post.renderedBody)}</article>
```

Avoid raw HTML for user content. Sanitization should happen before a value becomes trusted.

## Fragments

Components may return multiple sibling nodes:

```web
<h2>{title}</h2>
<p>{description}</p>
```

When a single wrapper is needed, use normal HTML:

```web
<section>
  <h2>{title}</h2>
  <p>{description}</p>
</section>
```

## Slots

Default slot:

```web
<slot />
```

## Named Slot

:::warning[Not Yet Implemented]
Named slots (`<slot name="..." />`) and `<template slot="...">` consumers are documented but not yet implemented in the current MVP runtime. Only default `<slot />` is currently supported.
:::

```web
<slot name="actions" />
```

Consumer:

```web
<Panel>
  <p>Panel body</p>

  <template slot="actions">
    <button>Save</button>
  </template>
</Panel>
```

