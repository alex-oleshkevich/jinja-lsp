---
name: "number"
category: "test"
signature: "number()"
since: "2.0"
---

Returns true if the value is any numeric type, including both integers and floats. Use this when the specific numeric subtype does not matter, only that the value is a number.

## Usage

```jinja
{% if value is number %}{{ value * 2 }}{% endif %}
```
