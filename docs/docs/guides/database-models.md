---
sidebar_position: 16
---

# Database Models

WebScript can describe SQLite database schema with `@model` files under
`app/models`. The first database slice supports schema generation and plain SQL
migrations. Runtime model helpers such as `User.find` are planned separately.

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
