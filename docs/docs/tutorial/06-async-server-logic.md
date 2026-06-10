---
sidebar_position: 6
title: Async server logic
---

# Async server logic

`@load` and `@action` support full async server code: functions, loops, exceptions, and builtins like `sleep`, `spawn`, `timeout`, and `fetch`.

## Functions, sleep, spawn, and timeout

Create `app/pages/async-demo.web`:

```web
@page "/async-demo"

@load {
  fn countUpTo(limit: int): int {
    current := 0
    while current < limit {
      current = current + 1
    }
    return current
  }

  steps: int = 0
  message: string = ""
  timedOut: bool = false

  try {
    _: object = await sleep(25ms)
    steps = countUpTo(5)

    task := spawn(sleep(2s))
    try {
      _: object = await timeout(100ms, task)
    } catch err {
      timedOut = err.message == "timeout"
    }

    if timedOut {
      message = "timeout fired after " + steps + " steps"
    }
    if !timedOut {
      message = "unexpected: task finished"
    }
  } catch err {
    throw("load failed: " + err.message)
  }
}

<main>
  <h1>Async demo</h1>
  <p>Steps: {steps}</p>
  <p>{message}</p>
</main>
```

- `sleep(duration)` — pause without blocking the runtime
- `spawn(task)` — run async work concurrently
- `timeout(duration, task)` — fail if the task does not finish in time

## Fetch and error handling

Create `app/pages/fetch-demo.web`:

```web
@page "/fetch-demo"

@load {
  origin: string = ""
  error: string = ""

  try {
    data: HttpBinGet = await fetch("https://httpbin.org/get", HttpBinGet)
    origin = data.origin
  } catch err {
    error = err.message
  }
}

<main>
  <h1>Fetch demo</h1>
  <p>Origin: {origin}</p>
  <p>Error: {error}</p>
</main>
```

`fetch` returns a response object with `ok`, `status`, and body helpers. Use `try/catch` for recoverable errors and `throw` to propagate.

## `@do` vs `@load`

| Block | Async | Use for |
|-------|-------|---------|
| `@load` | Yes | Data fetching, external APIs, timed work |
| `@action` | Yes | Mutations, auth, redirects |
| `@do` | No | Quick sync calculations in the template |

Full syntax details: [Server Logic](../language/server-logic).

## Streaming HTML (preview)

For slow sections of a page, `@defer` sends a placeholder first and streams replacement HTML when ready:

```web
@defer {
  stats: Stats = await analytics.getStats()
  <StatsCard stats={stats} />
} @placeholder {
  <StatsCardSkeleton />
}
```

See [Async Rendering](../guides/async-rendering) when you need streamed partial updates.

Next: wrap up and explore the rest of the docs.
