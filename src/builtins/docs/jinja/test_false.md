---
name: "false"
category: "test"
signature: "false()"
since: "2.0"
---

Returns true if the value is exactly `False`. Unlike a plain falsy check, this test distinguishes the boolean `False` from other falsy values such as `0`, `None`, or an empty string.

## Usage

```jinja
{% if value is false %}The flag is explicitly False.{% endif %}
```
