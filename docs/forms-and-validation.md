# Forms And Validation

WebScript treats forms as server-native. Forms submit to actions, inputs are parsed into typed values, validation failures are structured, and CSRF protection is handled by the runtime.

## Basic Form

```web
@action updateProfile(input: ProfileInput) -> Redirect {
  await auth.user.update(input)
  redirect("/account")
}

<form @submit={updateProfile}>
  <input name="name" value={auth.user.name} />
  <input name="email" type="email" value={auth.user.email} />
  <button>Save</button>
</form>
```

## Input Types

```web
type ProfileInput {
  name: string
  email: string
}
```

The runtime parses form fields into the declared input shape.

## Validation Rules

Recommended field annotations:

```web
type RegisterInput {
  email: string @email
  password: string @min(12)
  name: string @min(1) @max(120)
}
```

Validation runs before the action body. If validation fails, the action returns a structured `422` response.

## Optional Fields

```web
type SearchInput {
  q: string?
  page: int = 1
}
```

Defaults are applied when the field is missing.

## Field Errors

Actions can fail with field errors:

```web
fail({
  email: "Email is already in use"
})
```

Templates can render errors:

```web
@if form.errors.email {
  <p class="error">{form.errors.email}</p>
}
```

## Form State

The runtime should expose form state during enhanced submissions:

```web
<button disabled={form.pending}>
  {form.pending ? "Saving..." : "Save"}
</button>
```

Without JavaScript, normal server form behavior still works.

## CSRF

For session-authenticated forms, CSRF protection should be automatic.

The runtime can inject a hidden CSRF field:

```web
<form @submit={updateProfile}>
  @csrf
  <input name="name" />
  <button>Save</button>
</form>
```

Or include it automatically for `@submit` forms when auth/session is enabled.

## File Uploads

```web
type AvatarInput {
  avatar: File @maxSize(2mb) @mime("image/png", "image/jpeg")
}

@action uploadAvatar(input: AvatarInput) -> Redirect {
  path := await storage.put(input.avatar)
  await auth.user.update({ avatarPath: path })
  redirect("/account")
}
```

Forms with file inputs should automatically use multipart handling.

## Progressive Enhancement

A form should work without JavaScript:

```web
<form @submit={login}>
  <input name="email" />
  <input name="password" type="password" />
  <button>Login</button>
</form>
```

With the client runtime available, the same form can submit without a full page reload, preserve focus, and update form errors inline.

