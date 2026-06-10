---
sidebar_position: 6
---

# Server Logic

WebScript server blocks support standard imperative coding: functions, loops, exceptions, and async tasks.

## Where It Applies

| Block | Async | Constructs |
|-------|-------|------------|
| `@load` | Yes | Full syntax below |
| `@action` | Yes | Full syntax below |
| `@do` | No | Sync subset — no `await`, `fetch`, `spawn`, `timeout`, or `throw` |

## Variables

```web
count: int = 0
name := "Ada"
session.count = session.count + 1
result = { status: 404, body: "", error: "missing" }
```

## Functions

```web
fn fetchWithRetry(url: string): object {
  attempts := 0
  while attempts < 3 {
    try {
      response := await fetch(url)
      if response.status >= 400 {
        throw("HTTP " + response.status)
      }
      return response
    } catch err {
      attempts = attempts + 1
      if attempts >= 3 {
        throw(err.message)
      }
      await sleep(500ms)
    }
  }
  throw("unreachable")
}
```

## Control Flow

```web
if input.name == "" {
  fail("Name is required")
}

while attempts < 3 {
  attempts = attempts + 1
}
```

## Exceptions

`throw` is exceptional control flow caught by `try/catch`. `fail` is an expected action failure (422) and is **not** caught.

```web
try {
  response := await fetch("https://example.com/api")
  if !response.ok {
    throw("upstream returned " + response.status)
  }
} catch err {
  result = { error: err.message }
}
```

## Async Builtins

```web
await sleep(500ms)
task := spawn(fetch("https://example.com"))
result := await timeout(5s, task)
response := await fetch("https://example.com/api")
```

### `fetch(url)`

Returns:

```web
{
  status: int
  body: string
  ok: bool
}
```

Network errors reject into `try/catch`. HTTP 4xx/5xx do not auto-throw.

### `sleep(duration)`

Durations: `500ms`, `5s`, `1m`, `1h`, `1d`.

### `spawn(promise)` and `timeout(duration, promise)`

`spawn` returns a promise handle. `timeout` rejects with `"timeout"` if the promise does not resolve in time.
