---
name: "string"
category: "test"
signature: "string()"
since: "2.0"
---

Returns true if the value is a string. Use this to safely apply string-specific filters or operations without risking a type error on non-string values.

## Usage

```jinja
{% if value is string %}{{ value | upper }}{% endif %}
```
