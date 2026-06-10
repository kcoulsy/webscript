# Validation Schemas

WebScript can describe response shapes with `@schema` files under
`app/schemas/`. Pass a schema into `fetch()` or `db.query()` to validate data at
runtime and get typed results in `@load` / `@action` blocks.

## Defining a schema

```web
@schema ApiResponse {
  message: string @min(5) @max(255)
  email: string @email
  nickname: string @optional
}
```

### Field types

| Type | JSON / SQLite |
|------|----------------|
| `string` | text |
| `int` | whole numbers |
| `float` | numbers |
| `bool` | booleans |

### Field decorators

| Decorator | Meaning |
|-----------|---------|
| `@min(n)` | minimum string length or numeric value |
| `@max(n)` | maximum string length or numeric value |
| `@email` | basic email format |
| `@optional` | field may be missing or null |

Fields are required unless marked `@optional`.

## Using schemas with `fetch`

`fetch(url, Schema)` performs a GET request, parses the JSON body, validates it
against the schema, and returns the validated object.

```web
@load {
  data: ApiResponse = await fetch("https://api.example.com/user", ApiResponse)
}
```

- Non-2xx HTTP status codes throw.
- Invalid JSON or validation failures throw with a field-level message.

## Using schemas with `db.query`

`db.query` requires a schema as the last argument:

```web
@load {
  rows: TodoRow[] = await db.query("SELECT title, done FROM Todo", TodoRow)
  open: TodoRow[] = await db.query(
    "SELECT title, done FROM Todo WHERE done = ?",
    [false],
    TodoRow
  )
}
```

Each row is validated and coerced to the schema. The result type is
`SchemaName[]`.

`db.execute` is unchanged and does not take a schema.

## Checking schemas

`web check` validates schema files and ensures `fetch` / `db.query` calls include
a known schema name. Schema names must not collide with `@model` names.
