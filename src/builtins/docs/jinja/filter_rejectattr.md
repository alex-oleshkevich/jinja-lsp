---
name: "rejectattr"
category: "filter"
signature: "rejectattr(attribute, test, *args)"
since: "2.7"
params:
  - name: "attribute"
    type: "string"
    default: ""
    required: true
  - name: "test"
    type: "string"
    default: ""
    required: false
  - name: "args"
    type: "any"
    default: ""
    required: false
---

Filter a sequence of objects by rejecting those for which the specified attribute passes the given test. This is the inverse of `selectattr` and is useful for filtering out objects from a list based on an attribute condition.

## Usage

```jinja
{{ users | rejectattr('is_active') | list }}
{{ products | rejectattr('stock', 'equalto', 0) | list }}
```
