---
name: "callable"
category: "test"
signature: "callable()"
since: "2.0"
---

Returns true if the value is callable, meaning it can be called like a function. This includes functions, methods, and objects with a `__call__` method.

## Usage

```jinja
{% if value is callable %}{{ value() }}{% endif %}
```
