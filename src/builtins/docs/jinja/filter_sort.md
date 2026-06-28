---
name: "sort"
category: "filter"
signature: "sort(reverse=False, case_sensitive=False, attribute=None)"
since: "2.0"
params:
  - name: "reverse"
    type: "boolean"
    default: "False"
    required: false
  - name: "case_sensitive"
    type: "boolean"
    default: "False"
    required: false
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
---

Sort a sequence. By default items are sorted in ascending order; set `reverse=True` for descending. String sorting is case-insensitive by default. Provide `attribute` to sort objects by a specific attribute or dotted path.

## Usage

```jinja
{{ ['banana', 'apple', 'cherry'] | sort }}
{{ users | sort(attribute='last_name', reverse=True) }}
```
