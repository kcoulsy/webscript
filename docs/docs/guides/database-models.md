---
sidebar_position: 16
---

# Database Models

WebScript can describe SQLite database schema with `@model` files under
`app/models`. The database slice supports schema generation, plain SQL
migrations, typed model helpers, and a raw SQL helper for ad-hoc queries.

## Model Files

```web
@model User {
  id: int @primary @auto
  email: string @unique
  name: string
  createdAt: datetime @default(now)
  @index(email)
}

@model Post {
  id: int @primary @auto
  authorId: int @references(User.id) @relation(author)
  title: string
  slug: string @unique
  published: bool @default(false)
  @index(authorId, published)
}
```

Supported field types:

- `string`, `date`, `datetime` -> `TEXT`
- `int`, `bool` -> `INTEGER`
- `float` -> `REAL`
- `bytes` -> `BLOB`

Supported field decorators:

- `@primary`
- `@auto`
- `@unique`
- `@nullable`
- `@default(value)`
- `@references(Model.field)`
- `@relation(name)`

Supported model-level decorators:

- `@index(field, ...)`
- `@uniqueIndex(field, ...)`

Relations are explicit foreign-key columns. `@relation(name)` records the
developer-facing relation name, but the first implementation only emits the
foreign key.

## Generate Migrations

```bash
web db:generate create_posts
```

This scans `app/models/**/*.web`, generates deterministic SQLite DDL, writes a
new plain SQL migration in `db/migrations`, and stores the latest generated
schema in `db/schema.sql`. If the schema is unchanged, no migration is created.

## Apply Migrations

```bash
web db:migrate
```

Migrations run against `.web/data.sqlite` by default. Applied migrations are
tracked in `_webscript_migrations` with a checksum so edited historical
migrations fail fast instead of being silently skipped.

## Model Helpers

Model methods run against the same SQLite database and return promises that must
be awaited in `@load` or `@action` blocks:

```web
@load {
  todos: Todo[] = await Todo.all()
}

@action addTodo {
  await Todo.create(input)
  redirect("/todos")
}
```

Supported methods today: `all`, `find`, `create`, `update`, `deleteAll`.

## Raw SQL

Use `db.query` and `db.execute` when you need plain SQL instead of model
helpers. Both methods accept an optional second argument with bound parameters.

```web
@load {
  open: object[] = await db.query(
    "SELECT * FROM Todo WHERE done = ? ORDER BY createdAt",
    [false]
  )
}

@action archiveDone {
  _: object = await db.execute(
    "DELETE FROM Todo WHERE done = ?",
    [true]
  )
  redirect("/todos")
}
```

| Method | Purpose | Returns |
|--------|---------|---------|
| `db.query(sql, params?)` | Read rows | `object[]` with column names as keys |
| `db.execute(sql, params?)` | Run writes | `{ changes: int, lastInsertId: int }` |

- `params` is optional and defaults to `[]`.
- Use `?` placeholders for bound values.
- `NULL` columns are omitted from result objects.
- Raw SQL calls appear on the debug bar async timeline as `db.query(...)` /
  `db.execute(...)` bars, distinct from model calls.
