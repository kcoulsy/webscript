# Getting Started

This guide introduces the expected workflow for a WebScript project.

## Install

The framework is expected to provide a CLI named `web`.

```bash
web new my-app
cd my-app
web serve
```

The development server loads `.web` files directly. There is no required build step.

## Create A Page

Create a page file:

```txt
app/pages/index.web
```

```web
@page "/"

<main>
  <h1>Home</h1>
  <p>Welcome to WebScript.</p>
</main>
```

Visit `/`.

## Add Typed Data

```web
@page "/posts"

@load {
  posts: Post[] = await Post.published()
}

<main>
  <h1>Posts</h1>

  @for post in posts {
    <article>
      <h2>{post.title}</h2>
      <p>{post.excerpt}</p>
    </article>
  }
</main>
```

`@load` runs on the server before the main page template is rendered.

## Add An API Route

```web
@api GET "/api/posts" -> Json<Post[]>

@query {
  page: int = 1
}

@load {
  posts: Post[] = await Post.published().paginate(page)
}

return ok(posts)
```

API routes return JSON by default, but explicit response types are recommended for public endpoints.

## Add A Form Action

```web
@page "/login"

@body input: LoginForm

@action login(input: LoginForm) -> Redirect {
  user: User? = await User.findByEmail(input.email)

  if user == null || !password.verify(input.password, user.passwordHash) {
    fail("Invalid email or password")
  }

  await auth.login(user.id)

  redirect("/dashboard")
}

<form @submit={login}>
  <input name="email" type="email" required />
  <input name="password" type="password" required />
  <button>Login</button>
</form>
```

Actions run on the server. They can return redirects, JSON, HTML, or error responses.

## Protect A Page

```web
@page "/dashboard"

@auth required {
  redirect: "/login"
}

<h1>{auth.user.name}</h1>
```

The runtime reads the session cookie, verifies it, loads session data, and exposes typed `auth`.

## Add Async HTML

```web
@await db.users.recent() as users {
  <UserList users={users} />
} @loading {
  <Spinner />
} @error err {
  <p>Could not load users: {err.message}</p>
}
```

Use `@await` when a specific part of the template depends on async data.

## Add Streaming HTML

```web
@defer {
  stats: Stats = await analytics.getStats()

  <StatsCard stats={stats} />
} @placeholder {
  <StatsCardSkeleton />
}
```

The server can send the shell immediately, render the placeholder, then stream replacement HTML when the deferred block resolves.

