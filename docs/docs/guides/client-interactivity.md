# Client Interactivity

WebScript is server-first, but it supports browser-only interactivity through a tiny standard runtime.

## Implemented in MVP

The current runtime supports:

- `@client` signal declarations (`signal<int>`, `signal<bool>`, `signal<string>`)
- `@client` named handler functions (`fn save() { ... }`)
- Event handlers: `@click`, `@input`, `@change`, `@submit`, `@keydown`, `@keyup`, `@focus`, `@blur`
- Event modifiers: `.prevent` (calls `event.preventDefault()`), `.stop` (calls `event.stopPropagation()`)
- Pipe-lambda handlers (`|event| body`) as the canonical handler form
- General client expressions in handlers (assignments, arithmetic, `event.key`, string concat, `&&`, etc.)
- Bare expressions as sugar (desugared to `|event| ...` internally)
- Reactive `@if` blocks driven by `signal<bool>` (both branches stay in the DOM)
- Server-rendered initial HTML with per-island hydration scripts
- `/.web/runtime.js` signal primitive (no bundler)

Pipe-lambda handlers (canonical form):

```web
@click={|event| count++}
@click={|event| { count = 0; status = "reset" }}
@input={|event| name = event.target.value}
@change={|event| note = event.target.value}
@submit.prevent={|event| save()}
@keydown={|event| event.key == "Enter" && save()}
@focus={|event| status = "focused"}
@blur={|event| status = "blurred"}
```

Bare expressions still work as sugar:

```web
@click={count++}
@click={count = 0}
@submit.prevent={save}
```

Named handlers:

```web
@client {
  count: signal<int> = 0

  fn save() {
    count = count + 1
  }
}

<button @click={save}>Save</button>
```

See the `/counter` demo page for counters, toggle panels, live text input, and form/event demos.

Coming soon: reactive `@for` and page-level `@client` blocks.

## Runtime Script

The framework can serve a built-in client runtime:

```html
<script src="/.web/runtime.js"></script>
```

This runtime hydrates small interactive islands without requiring a bundler.

## Soft Navigation

Same-origin link clicks are intercepted by `WebScript.navigate` in `/.web/runtime.js`. The runtime:

1. Fetches the next page as HTML
2. Swaps only the `[data-ws-outlet]` region (page content at the layout `<slot />`)
3. Optionally swaps `[data-ws-nav-region]` when layout chrome changes (for example login/logout links)
4. Updates the document title and page-scoped styles
5. Re-runs island hydration scripts for the new page

Page island state is reset on each navigation. Layout chrome outside the outlet is left in place.

Opt out of soft navigation when needed:

```html
<a href="/defer-demo" data-ws-nav="reload">Defer demo</a>
```

`@defer` pages automatically fall back to a full document load because streaming placeholders are not swapped client-side yet.

`WebScript.action()` redirects use soft navigation when possible.

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

Handlers are functions. Use pipe-lambda syntax for inline handlers:

```web
<button @click={|event| count++}>Increment</button>
<input @input={|event| name = event.target.value} />
<form @submit.prevent={|event| save()}>Save</form>
```

Use UI primitives instead of raw markup in client islands:

```web
<UI.Button label="+" variant="outline" size="sm" @click={count++} />
<UI.Input id="name" value={name} @input={|event| name = event.target.value} />
```

Events and `class` on component calls are forwarded to the primitive's `data-ws-bind` target.

Shorthand sugar (desugared to `|event| ...`):

```web
<button @click={count++}>Increment</button>
<form @submit.prevent={save}>Save</form>
```

Common events (all implemented on component islands):

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

Modifiers (chain after the event name):

```web
@submit.prevent={save}
@click.stop={select}
@submit.prevent.stop={save}
```

Inside handlers you can use `event`, `event.target.value`, `event.key`, `event.preventDefault()`, and `event.stopPropagation()`.

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

:::warning[Not Yet Implemented]
Enhanced form features (`form.pending`, `form.errors.fieldName`, `<form @submit={actionName}>` binding, and progressive enhancement without page reload) are documented but not yet implemented in the current MVP runtime. Forms currently use the standard HTML POST with `_action` field pattern.
:::

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

