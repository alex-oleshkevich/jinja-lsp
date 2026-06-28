---
name: "string"
category: "filter"
signature: "string()"
since: "2.0"
params: []
---

Convert a value to a string. If the environment uses Unicode, the result will be a Unicode string. This is equivalent to calling `str()` in Python and is useful for ensuring a value is treated as text before applying string filters.

## Usage

```jinja
{{ 42 | string }}
{{ my_object | string | upper }}
```
