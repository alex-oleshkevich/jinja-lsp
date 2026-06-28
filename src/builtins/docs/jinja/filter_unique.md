---
name: "unique"
category: "filter"
signature: "unique(case_sensitive=False, attribute=None)"
since: "2.7"
params:
  - name: "case_sensitive"
    type: "boolean"
    default: "False"
    required: false
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
---

Yield unique values from a sequence, preserving the order of first occurrence. When `attribute` is provided, uniqueness is determined by that attribute on each object. String comparisons are case-insensitive unless `case_sensitive=True`.

## Usage

```jinja
{{ ['a', 'b', 'A', 'c'] | unique | list }}
{{ articles | unique(attribute='author_id') | list }}
```
