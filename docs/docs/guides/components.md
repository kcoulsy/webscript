# Components

Components are reusable `.web` units with typed props, optional slots, and template markup.

## Basic Component

```web
@component UserCard {
  user: User
}

<article class="user-card">
  <h2>{user.name}</h2>
  <p>{user.email}</p>
</article>
```

Use it from another template:

```web
<UserCard user={user} />
```

## Props

Props are typed:

```web
@component Avatar {
  name: string
  size: int = 40
  rounded: bool = true
}
```

Props with defaults are optional:

```web
<Avatar name={name} />
<Avatar name={name} size=80 />
```

## Required Props

Props without defaults are required:

```web
@component ErrorBox {
  message: string
}
```

This is invalid:

```web
<ErrorBox />
```

The current MVP supports self-closing component calls with string, int, bool, and simple identifier expression props:

```web
@component PostPreview {
  title: string
  rank: int = 0
  featured: bool = false
}

<article>
  @if featured {
    <strong>Featured</strong>
  }
  <h3>{title}</h3>
  <p>Rank {rank}</p>
</article>
```

Use it from a page:

```web
@page "/"

@let posts: string[] = ["One", "Two", "Three"]

<main>
  <PostPreview title="Pinned release notes" rank=1 featured=true />

  @for post in posts {
    <PostPreview title={post} />
  }
</main>
```

## Slots

Use `<slot />` for children:

```web
@component Panel {
  title: string
}

<section class="panel">
  <header>{title}</header>
  <div>
    <slot />
  </div>
</section>
```

Consumer:

```web
<Panel title="Billing">
  <BillingSummary account={account} />
</Panel>
```

## Named Slots

```web
@component Toolbar {}

<div class="toolbar">
  <div class="toolbar-main">
    <slot />
  </div>
  <div class="toolbar-actions">
    <slot name="actions" />
  </div>
</div>
```

Consumer:

```web
<Toolbar>
  <h1>Posts</h1>

  <template slot="actions">
    <a href="/posts/new">New post</a>
  </template>
</Toolbar>
```

## Server Components By Default

Components render on the server by default. They can access server-only data if passed in as props or loaded in server logic.

```web
@component RecentUsers {}

@load {
  users: User[] = await User.recent()
}

@for user in users {
  <UserCard user={user} />
}
```

## Client Components

Use `@client` for browser-only state or event handlers:

```web
@component Counter {}

@client {
  count: signal<int> = 0
}

<button @click={count++}>
  {count}
</button>
```

Only values declared in `@client` and serializable props should cross into the browser runtime.

## Component Naming

Component tags should be capitalized:

```web
<UserCard />
```

Lowercase tags are treated as HTML:

```web
<article></article>
```

## Namespaced Components

Organize components in subfolders and declare a qualified name with dot notation:

```web
# app/components/UI/button.web
@component UI.Button {
  label: string
}

<button>{label}</button>
```

Use the qualified name in templates:

```web
<UI.Button label="Save" />
```

Each dot-separated segment must be PascalCase (`UI.Button`, `UI.Forms.TextInput`). Folders under `app/components/` are for organization only — the registry key comes from the `@component` declaration, not the file path.

## Component Boundaries

A component should own:

- Its props.
- Its internal derived values.
- Its local markup.
- Its scoped style.
- Its client behavior, if any.

It should not mutate parent state directly. Use actions, events, or form submissions for mutations.
