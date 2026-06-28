---
name: "escape"
category: "filter"
signature: "escape()"
since: "2.0"
params: []
---

Convert special HTML characters (`&`, `<`, `>`, `"`, `'`) in a string to their safe HTML entity equivalents and mark the result as safe. Also available as the alias `e`. This is the primary defense against cross-site scripting (XSS) when outputting user-supplied content.

## Usage

```jinja
{{ user_input | escape }}
{{ user_input | e }}
```

In autoescape mode this filter is applied automatically to all variables, but you can also call it explicitly in non-autoescape templates to selectively protect output.
