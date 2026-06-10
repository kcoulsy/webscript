# Client Interactivity

WebScript is server-first, but it supports browser-only interactivity through a tiny standard runtime.

## Implemented in MVP

The current runtime supports:

- `@client` signal declarations (`signal<int>`, `signal<bool>`)
- `@click` handlers on component islands
- Server-rendered initial HTML with per-island hydration scripts
- `/.web/runtime.js` signal primitive (no bundler)

Coming soon: reactive `@if` / `@for`, additional events (`@input`, `@submit`), enhanced forms, and page-level `@client` blocks.

## Runtime Script

The framework can serve a built-in client runtime:

```html
<script src="/.web/runtime.js"></script>
```

This runtime hydrates small interactive islands without requiring a bundler.

## `@client`

Use `@client` for browser state and event handlers:

```web
@client {
  count: signal<int> = 0
}

<button @click={count++}>
  {count}
</button>
```

The initial HTML is still rendered on the server. The runtime activates the interactive parts in the browser.

## Signals

Signals hold reactive state:

```web
@client {
  open: signal<bool> = false
}

<button @click={open = !open}>Toggle</button>

@if open {
  <nav>...</nav>
}
```

Only client-safe values can be used in `@client`.

## Event Handlers

```web
<button @click={count++}>Increment</button>
<input @input={name = event.value} />
<form @submit={save}>Save</form>
```

Common events:

```web
@click
@input
@change
@submit
@keydown
@keyup
@focus
@blur
```

## Client And Server Boundaries

Server-only values cannot be used directly in client code:

```web
@client {
  // Invalid if db is server-only
  users = await db.users.all()
}
```

Use actions or API routes for client-to-server mutations:

```web
@action increment() -> Json<{ count: int }> {
  count := await Counter.increment(auth.user.id)
  return ok({ count: count })
}
```

## Enhanced Forms

Forms work without JavaScript and can be enhanced by the runtime:

```web
<form @submit={saveProfile}>
  <input name="name" value={auth.user.name} />
  <button disabled={form.pending}>Save</button>
</form>
```

With runtime enhancement:

- Submit without full page reload.
- Preserve scroll and focus.
- Render field errors inline.
- Follow redirects.
- Update streamed regions when needed.

## Hydration Islands

Interactive regions are scoped:

```web
<Counter />
<SearchBox />
```

Each component with `@client` becomes an island. Static server-rendered components do not hydrate.

## Serialization

Props passed to client islands must be serializable:

```web
<Counter initial={5} />
```

Do not pass database connections, request objects, secrets, or server-only closures.

## No Bundler Required

The runtime should discover client metadata from the `.web` file and hydrate it dynamically. A later optimizer may precompile client islands, but the default workflow remains no-build.

