# Examples

This page collects complete examples that combine multiple WebScript features.

## Dashboard With Deferred Stats

:::warning[Not Yet Implemented]
This example uses `@auth required { redirect }`, `auth.user`, and `@load` with `auth` context — these auth features are documented but not yet implemented in the current MVP runtime.
:::

```web
@page "/dashboard"

@auth required {
  redirect: "/login"
}

@load {
  user: User = auth.user
}

<main>
  <h1>{user.name}</h1>

  @let showAdmin: bool = user.role == "admin"

  @if showAdmin {
    <AdminLinks />
  }

  @defer {
    stats: Stats = await analytics.getStats(user.id)

    <StatsCard stats={stats} />
  } @placeholder {
    <StatsCardSkeleton />
  } @error err {
    <ErrorBox message="Could not load stats" />
  }
</main>
```

## Login And Logout

:::warning[Not Yet Implemented]
This example uses `@guest`, `@action` with typed inputs, `crypto.verifyPassword`, `auth.login()`, and `auth.logout()` — these auth features are documented but not yet implemented in the current MVP runtime.
:::

```web
@page "/login"

@guest {
  redirect: "/dashboard"
}

@action login(input: LoginForm) -> Redirect {
  user: User? = await User.findByEmail(input.email)

  if user == null || !crypto.verifyPassword(input.password, user.passwordHash) {
    fail("Invalid email or password")
  }

  await auth.login(user.id)

  redirect("/dashboard")
}

<form @submit={login}>
  <input name="email" type="email" autocomplete="email" />
  <input name="password" type="password" autocomplete="current-password" />
  <button>Login</button>
</form>
```

```web
@action logout() -> Redirect {
  await auth.logout()
  redirect("/login")
}

<form @submit={logout}>
  <button>Logout</button>
</form>
```

## Recent Orders Section

:::warning[Not Yet Implemented]
This example uses `@await` / `@loading` / `@error` — documented but not yet implemented. Use `@defer` with `@placeholder` instead.
:::

```web
<section>
  <h2>Recent orders</h2>

  @let limit: int = 5
  @let ordersPromise: Promise<Order[]> = db.orders.recent(limit)

  @await ordersPromise as orders {
    @for order in orders {
      <OrderRow order={order} />
    }
  } @loading {
    <OrderSkeleton count={limit} />
  } @error err {
    <ErrorBox message="Could not load orders" />
  }
</section>
```

## Typed Component Props

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

## Create Post API

:::warning[Not Yet Implemented]
This example uses `@api`, `@require auth`, `@body`, `created()`, `.header()`, and `.cookie()` — documented but not yet implemented in the current MVP runtime.
:::

```web
@api POST "/api/posts" -> Json<Post>

@require auth

@body input: CreatePost

@action {
  post := await Post.create {
    title: input.title
    slug: slug(input.title)
    content: input.content
    authorId: auth.user.id
  }

  return created(post)
    .header("Location", "/api/posts/{post.id}")
    .cookie("last_post_id", post.id, {
      httpOnly: true
      secure: true
      sameSite: "lax"
      maxAge: 1d
    })
}
```

## Preview Page With No-Store Header

:::warning[Not Yet Implemented]
This example uses `@headers` and `html.trusted()` — documented but not yet implemented in the current MVP runtime.
:::

```web
@page "/preview/{token:string}"

@headers {
  "Cache-Control": "no-store"
}

@load {
  preview := await Preview.find(token)

  if preview == null {
    return notFound()
  }
}

<h1>{preview.title}</h1>
<article>{html.trusted(preview.renderedBody)}</article>
```

## Counter Island

```web
@component Counter {
  initial: int = 0
}

@client {
  count: signal<int> = initial
}

<button @click={count++}>
  {count}
</button>
```

## Auth And Session Configuration

:::warning[Not Yet Implemented]
`@auth` and `@session` configuration blocks with typed session data are documented but not yet implemented. The current MVP uses a simple in-memory session. See [Auth and Sessions](./auth-and-sessions) for details.
:::

```web
@auth {
  mode: "stateful-session"
  driver: "session"
  cookie: "__Host_app_session"
  sameSite: "lax"
  secure: true
  httpOnly: true
  store: kv("sessions")
  ttl: 30d
}

@session {
  store: kv("sessions")
  ttl: 30d

  data {
    userId: UserId
    roles: string[]
    csrfToken: string
  }
}
```
