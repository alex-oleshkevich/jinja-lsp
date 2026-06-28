---
name: "integer"
category: "test"
signature: "integer()"
since: "2.0"
---

Returns true if the value is an integer. Use this to distinguish integers from floats or other numeric types when type-specific behavior is needed.

## Usage

```jinja
{% if value is integer %}{{ value }} is a whole number.{% endif %}
```
