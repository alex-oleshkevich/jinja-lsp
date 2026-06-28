---
name: "abs"
category: "filter"
signature: "abs()"
since: "2.6"
params: []
---

Return the absolute value of a number. Equivalent to Python's built-in `abs()` function.

## Usage

```jinja
{{ -42 | abs }}
```

Useful when you need to display a magnitude without a sign, for example when showing the difference between two values.
