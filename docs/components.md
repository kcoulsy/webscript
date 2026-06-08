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
  user: User
  size: int = 40
  rounded: bool = true
}
```

Props with defaults are optional:

```web
<Avatar user={user} />
<Avatar user={user} size={80} />
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

## Component Boundaries

A component should own:

- Its props.
- Its internal derived values.
- Its local markup.
- Its scoped style.
- Its client behavior, if any.

It should not mutate parent state directly. Use actions, events, or form submissions for mutations.

