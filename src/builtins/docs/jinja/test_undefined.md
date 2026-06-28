---
name: "undefined"
category: "test"
signature: "undefined()"
since: "2.0"
---

Returns true if the variable is undefined, meaning it was not set in the current context. This is the inverse of the `defined` test and can be used to provide fallback content when a variable is missing.

## Usage

```jinja
{% if value is undefined %}No value was provided.{% endif %}
```
