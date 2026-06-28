---
name: "max"
category: "filter"
signature: "max(attribute=None, case_sensitive=False)"
since: "2.0"
params:
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
  - name: "case_sensitive"
    type: "boolean"
    default: "False"
    required: false
---

Return the largest item from a sequence. When `attribute` is provided, comparison is performed on that attribute of each object rather than the object itself. String comparisons are case-insensitive by default.

## Usage

```jinja
{{ [1, 5, 3, 2] | max }}
{{ users | max(attribute='age') }}
```
