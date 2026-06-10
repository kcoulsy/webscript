# Project Structure

WebScript projects are designed to be understandable from the filesystem. A typical project can look like this:

```txt
my-app/
  app/
    pages/
      index.web
      dashboard.web
      account.web
    api/
      posts.web
    components/
      UserCard.web
      ErrorBox.web
    models/
      User.web
      Post.web
    layouts/
      AppLayout.web
    actions/
      auth.web
  public/
    favicon.ico
    images/
  styles/
    app.css
  db/
    migrations/
  web.config
```

The exact layout is flexible, but the runtime should support these conventions.

## `app/pages`

Page files render HTML responses by default.

```web
@page "/account"

<h1>Your account</h1>
```

Pages can include `@load`, `@action`, guards, headers, async blocks, components, and markup.

## `app/api`

API files define JSON-oriented routes.

```web
@api GET "/api/posts"

return ok(await Post.published())
```

An API file can define one route or multiple related routes, depending on framework convention.

## `app/components`

Components are reusable `.web` files that render markup and accept typed props.

```web
@component UserCard {
  user: User
}

<article>
  <h2>{user.name}</h2>
  <p>{user.email}</p>
</article>
```

Use components from pages or other components:

```web
<UserCard user={user} />
```

## `app/models`

Model files describe database schema for migration generation.

```web
@model User {
  id: int @primary @auto
  email: string @unique
  name: string
  createdAt: datetime @default(now)
  @index(email)
}
```

Model files do not render routes or components. They are read by `web db:generate`.

## `app/layouts`

Layouts wrap pages in shared HTML.

```web
@layout AppLayout {
  title: string = "App"
}

<!doctype html>
<html>
  <head>
    <title>{title}</title>
  </head>
  <body>
    <slot />
  </body>
</html>
```

Pages can select a layout:

```web
@page "/dashboard"
@layout AppLayout { title: "Dashboard" }
```

## `public`

Static files are served as-is.

```txt
public/logo.png -> /logo.png
```

Files in `public` should not require compilation.

## `styles`

Global styles can live in `styles`. Component-scoped styles can live inside `.web` files or beside components.

## `db/migrations`

Generated and hand-authored migration files live in `db/migrations`.
Migration files are plain SQL and are applied in filename order by `web db:migrate`.

## `web.config`

Project-level configuration can define auth defaults, deployment mode, runtime adapter, database bindings, and environment settings.

```web
@deploy {
  mode: "runtime"
}

@auth {
  mode: "stateful-session"
  store: kv("sessions")
}
```

## Route Discovery

Route discovery can be explicit, file-based, or mixed.

Explicit:

```web
@page "/posts/{slug:string}"
```

File-based:

```txt
app/pages/posts/[slug].web -> /posts/{slug}
```

Explicit route declarations should take precedence because they are unambiguous and self-documenting.
