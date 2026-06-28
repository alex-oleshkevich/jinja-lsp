---
name: "min"
category: "filter"
signature: "min(attribute=None, case_sensitive=False)"
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

Return the smallest item from a sequence. When `attribute` is provided, comparison is performed on that attribute of each object rather than the object itself. String comparisons are case-insensitive by default.

## Usage

```jinja
{{ [4, 1, 3, 2] | min }}
{{ products | min(attribute='price') }}
```
