# Styling And Assets

WebScript should support plain CSS, scoped component styles, static assets, and runtime-served framework assets without requiring a bundler.

## Global CSS

Reference global CSS from a layout:

```web
<link rel="stylesheet" href="/styles/app.css" />
```

Files in `public` are served directly:

```txt
public/styles/app.css -> /styles/app.css
```

## Component Styles

Components can define scoped styles:

```web
@component UserCard {
  user: User
}

<article class="card">
  <h2>{user.name}</h2>
</article>

@style {
  .card {
    border: 1px solid #ddd;
    padding: 1rem;
  }
}
```

Scoped styles only affect markup emitted by that component. Use `@style global { }` for page-wide CSS.

## Page Styles

```web
@page "/dashboard"

<main class="dashboard">
  ...
</main>

@style {
  .dashboard {
    display: grid;
    gap: 1rem;
  }
}
```

`@style { }` is scoped by default. Use `@style global { }` when styles should apply site-wide.

## Static Assets

```web
<img src="/images/logo.png" alt="Logo" />
```

Assets in `public` should be served without transformation.

## Asset Helper

For hashed or deployed assets:

```web
<img src={asset("images/logo.png")} alt="Logo" />
```

In no-build mode, `asset(...)` can resolve to `/images/logo.png`. In snapshot mode, it can resolve to a fingerprinted URL.

## Runtime Assets

The WebScript runtime can expose internal assets under:

```txt
/.web/runtime.js
/.web/stream.js
/.web/dev.css
```

These paths are reserved by the framework.

## CSS Without Bundling

The default model is:

- Plain CSS files work directly.
- Component-scoped CSS is processed lazily by the runtime.
- Optional snapshot mode can extract and fingerprint CSS.
- No external bundler is required.

## Inline Styles

Use inline styles sparingly:

```web
<div style={{ width: "{progress}%" }}></div>
```

Prefer classes for maintainability.

