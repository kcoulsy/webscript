# Layouts

Layouts wrap pages in shared HTML structure. They are useful for document shells, navigation, metadata, scripts, styles, and common page chrome.

## Define A Layout

```web
@layout AppLayout {
  title: string = "App"
}

<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{title}</title>
    <link rel="stylesheet" href="/styles/app.css" />
  </head>
  <body>
    <AppNav />
    <slot />
    <script src="/.web/runtime.js"></script>
  </body>
</html>
```

## Use A Layout

```web
@page "/dashboard"

@layout AppLayout {
  title: "Dashboard"
}

<main>
  <h1>Dashboard</h1>
</main>
```

The page content is inserted at `<slot />`.

## Layout Props

Layouts accept typed props:

```web
@layout MarketingLayout {
  title: string
  description: string?
}
```

Use them from a page:

```web
@layout MarketingLayout {
  title: "Pricing"
  description: "Simple pricing for teams."
}
```

## Nested Layouts

Layouts can compose:

```web
@layout AccountLayout {
  title: string
}

@layout AppLayout {
  title: title
}

<main class="account">
  <AccountSidebar />
  <section>
    <slot />
  </section>
</main>
```

## Default Layout

A project can configure a default layout:

```web
@defaults {
  layout: AppLayout
}
```

Pages can override it:

```web
@layout MarketingLayout { title: "Home" }
```

Or opt out:

```web
@layout none
```

## Metadata

Layouts can render metadata passed by pages:

```web
@layout AppLayout {
  title: string
  description: string? = null
}

<head>
  <title>{title}</title>

  @if description != null {
    <meta name="description" content={description} />
  }
</head>
```

## Streaming With Layouts

Layouts can contain deferred sections:

```web
<body>
  <AppNav />

  <slot />

  @defer {
    <FooterStats />
  } @placeholder {
    <FooterSkeleton />
  }
</body>
```

Deferred blocks inside layouts behave the same as deferred blocks inside pages.

