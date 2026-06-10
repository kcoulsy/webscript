# Data Loading And Actions

WebScript separates data reads from mutations while keeping both close to the markup that uses them.

## `@load`

`@load` runs on the server before the main page template is rendered.

```web
@page "/posts"

@load {
  posts: Post[] = await Post.published()
}

@for post in posts {
  <PostCard post={post} />
}
```

Values declared in `@load` are available to the page template.

`@load` supports full server logic — functions, loops, try/catch, throw, and async builtins (`fetch`, `sleep`, `spawn`, `timeout`). See [Server Logic](../language/server-logic).

Example with `fetch` and error handling:

```web
@load {
  error: string = ""

  try {
    response := await fetch("https://httpbin.org/status/404")
    if !response.ok {
      throw("upstream returned " + response.status)
    }
  } catch err {
    error = err.message
  }
}
```

## Returning From `@load`

`@load` can return a response:

```web
@load {
  post := await Post.find(slug)

  if post == null {
    return notFound()
  }
}
```

Returning a response stops normal page rendering.

## `@action`

Actions handle server-side mutations:

```web
@action publish(input: PublishPost) -> Redirect {
  post := await Post.find(input.id)

  if post == null {
    return notFound()
  }

  await post.publish()

  redirect("/posts/{post.slug}")
}
```

Actions can be submitted from forms:

```web
<form @submit={publish}>
  <input type="hidden" name="id" value={post.id} />
  <button>Publish</button>
</form>
```

The current MVP supports a small server-rendered form slice. A page can declare an action block, submit a POST form with `_action`, mutate session fields, and redirect:

```web
@page "/"

@action increment {
  session.count = session.count + 1
  redirect("/")
}

<p>Session count: {session.count}</p>
<form method="post" action="/">
  <input type="hidden" name="_action" value="increment" />
  <button>Increment</button>
</form>
```

Submitted form fields are exposed as strings on `input`:

```web
@action rememberName {
  if input.name == "" {
    fail("Name is required")
  }
  session.name = input.name
  redirect("/")
}

<form method="post" action="/">
  <input type="hidden" name="_action" value="rememberName" />
  <input name="name" value={session.name} />
  <button>Remember</button>
</form>
```

Action statements supported today:

- `if condition { ... }`
- `session.name = expression`
- `fail("message")`
- `redirect("/path")`

The dev runtime stores sessions in memory and sends a `webscript_session` HttpOnly cookie.

## Action Inputs

Inputs should be typed:

```web
type LoginForm {
  email: string
  password: string
}

@action login(input: LoginForm) -> Redirect {
  // ...
}
```

The runtime parses form data or JSON into the input type.

## Failing An Action

Use `fail(...)` for validation or business-rule failures:

```web
if user == null {
  fail("Invalid email or password")
}
```

Structured failures:

```web
fail({
  email: "No account exists for this email"
})
```

Failures should return a `422 Unprocessable Entity` response by default for forms and APIs.

## Redirect After Mutation

Use redirects after successful form mutations:

```web
await Post.create(input)
redirect("/posts")
```

This supports the post-redirect-get pattern and avoids accidental resubmits.

## Actions Returning JSON

```web
@action saveDraft(input: DraftInput) -> Json<Draft> {
  draft := await Draft.save(input)
  return ok(draft)
}
```

This is useful for client-enhanced forms and API-like interactions.

## Auth In Actions

```web
@action createPost(input: CreatePost) -> Redirect {
  require(auth.check)

  post := await Post.create {
    title: input.title
    authorId: auth.user.id
  }

  redirect("/posts/{post.slug}")
}
```

Prefer route-level guards when the whole page or API requires auth:

```web
@require auth
```

## Transactions

The runtime should provide a transaction helper:

```web
@action checkout(input: CheckoutInput) -> Redirect {
  order := await db.transaction {
    cart := await Cart.current(auth.user.id)
    order := await Order.fromCart(cart)
    await cart.clear()
    order
  }

  redirect("/orders/{order.id}")
}
```

## Idempotency

For payments, webhooks, and retryable forms, actions should support idempotency keys:

```web
@action charge(input: ChargeInput) -> Json<ChargeResult> {
  @idempotent input.idempotencyKey

  result := await billing.charge(input)
  return ok(result)
}
```
