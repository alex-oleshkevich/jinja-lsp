---
name: "upper"
category: "test"
signature: "upper()"
since: "2.0"
---

Returns true if all cased characters in the string are uppercase. The string must contain at least one cased character; purely numeric or symbol strings will return false.

## Usage

```jinja
{% if value is upper %}Already uppercase.{% else %}{{ value | upper }}{% endif %}
```
