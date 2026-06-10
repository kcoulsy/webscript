---
sidebar_position: 3
title: Components
---

# Components

Components are reusable `.web` files with typed props. They live in `app/components/`.

## Create `PostPreview`

Create `app/components/PostPreview.web`:

```web
@component PostPreview {
  title: string
  rank: int = 0
  featured: bool = false
}

<article class="post-preview">
  @if featured {
    <strong>Featured</strong>
  }
  <h3>{title}</h3>
  <p>Rank {rank}</p>
</article>
```

Props without defaults are required. Props with defaults are optional.

## Use the component on a page

Back in `app/pages/index.web`:

```web
@page "/"

@let author = { name: "Ada", role: "admin" }
@let visits = 5
@let posts = [
  { title: "Intro", slug: "intro", featured: true },
  { title: "Launch", slug: "launch", featured: false }
]

<main>
  <h1>My Posts</h1>

  <PostPreview
    title={"Pinned for " + author.name}
    rank={visits - 4}
    featured={true}
  />

  @for post in posts {
    <PostPreview title={post.title} featured={post.featured} />
  }
</main>
```

Components are referenced by file name. Expression props can be strings, ints, bools, or simple identifiers.

## When to use components

- Repeated markup (cards, badges, nav items)
- Typed, documented UI boundaries
- Slots and layouts for shared chrome (see the [Components guide](../guides/components))

Next you will load server data before rendering and add dynamic routes.
