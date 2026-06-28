---
name: "sum"
category: "filter"
signature: "sum(attribute=None, start=0)"
since: "2.0"
params:
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
  - name: "start"
    type: "number"
    default: "0"
    required: false
---

Return the sum of all values in a sequence. When `attribute` is provided, the values of that attribute are summed across all objects. The `start` parameter is added to the total, defaulting to zero.

## Usage

```jinja
{{ [1, 2, 3, 4, 5] | sum }}
{{ cart_items | sum(attribute='price') }}
```
