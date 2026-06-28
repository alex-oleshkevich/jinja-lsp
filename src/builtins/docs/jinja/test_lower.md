---
name: "lower"
category: "test"
signature: "lower()"
since: "2.0"
---

Returns true if all cased characters in the string are lowercase. The string must contain at least one cased character; purely numeric or symbol strings will return false.

## Usage

```jinja
{% if value is lower %}Already lowercase.{% else %}{{ value | lower }}{% endif %}
```
