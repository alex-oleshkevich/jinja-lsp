---
name: "boolean"
category: "test"
signature: "boolean()"
since: "2.11"
---

Returns true if the value is a boolean, meaning it is either `True` or `False`. This is useful for distinguishing booleans from other truthy or falsy values like integers or empty strings.

## Usage

```jinja
{% if value is boolean %}The value is a boolean.{% endif %}
```
