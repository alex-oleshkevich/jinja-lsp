---
name: "dict"
category: "function"
signature: "dict(**kwargs)"
since: "2.0"
params:
  - name: "kwargs"
    type: "kwargs"
    required: false
---

Create a dict from keyword arguments. Useful when you cannot use the `{key: value}` syntax directly.

## Usage

```jinja
{{ dict(a=1, b=2) }}
```
