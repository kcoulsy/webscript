---
sidebar_position: 1
title: Project setup
---

# Project setup

This tutorial builds a small posts app that exercises the main WebScript features: pages, components, routing, data loading, actions, control flow, and async server logic.

## Install the CLI

WebScript ships a `web` CLI. From this repository you can run it with Cargo:

```bash
cargo run --bin web -- new my-posts-app
cd my-posts-app
cargo run --bin web -- serve --port 3000
```

Or, once installed globally:

```bash
web new my-posts-app
cd my-posts-app
web serve
```

There is **no required build step**. The dev server loads `.web` files directly, parses and type-checks them, and serves the result.

## Project layout

A typical app looks like this:

```txt
my-posts-app/
  app/
    pages/          # HTML routes (@page)
    components/     # Reusable UI (@component)
    api/            # JSON routes (@api) — optional
    layouts/        # Shared HTML shells (@layout) — optional
  public/           # Static files (favicon, images)
  styles/           # Global CSS — optional
```

Pages live in `app/pages/`. Each file declares its route with `@page` and can mix server logic with HTML markup in one file.

## Create the home page

Create `app/pages/index.web`:

```web
@page "/"

<main>
  <h1>My Posts</h1>
  <p>Welcome to WebScript.</p>
</main>
```

Visit `http://localhost:3000/`. Edit the file, refresh the browser — changes appear immediately.

## Verify routes

List discovered routes:

```bash
web routes
```

Type-check the project:

```bash
web check
```

Next you will add variables, expressions, and control flow to this page.
