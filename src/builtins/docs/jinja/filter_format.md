---
name: "format"
category: "filter"
signature: "format(*args, **kwargs)"
params: []
---

Apply Python's `%` string formatting to the value. Positional and keyword arguments passed to the filter are substituted into the format string, following the same rules as Python's `printf`-style string formatting.

## Usage

```jinja
{{ "%s has %d items" | format(user.name, cart.count) }}
```

For most new templates the `format` filter can be replaced by Jinja2's own variable interpolation inside strings, but it remains useful when dealing with pre-existing format strings or when compatibility with Python format codes is required.
