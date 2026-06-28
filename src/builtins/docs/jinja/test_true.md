---
name: "true"
category: "test"
signature: "true()"
since: "2.0"
---

Returns true if the value is exactly `True`. Unlike a plain truthy check, this test distinguishes the boolean `True` from other truthy values such as `1`, `"yes"`, or a non-empty list.

## Usage

```jinja
{% if value is true %}The flag is explicitly True.{% endif %}
```
