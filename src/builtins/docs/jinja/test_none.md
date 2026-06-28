---
name: "none"
category: "test"
signature: "none()"
since: "2.0"
---

Returns true if the value is `None`. This is more explicit than a falsy check, since it distinguishes `None` from other falsy values like `False`, `0`, or an empty string.

## Usage

```jinja
{% if value is none %}No value provided.{% endif %}
```
