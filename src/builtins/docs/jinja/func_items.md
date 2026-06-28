---
name: "items"
category: "function"
signature: "items(mapping)"
since: "3.1"
params:
  - name: "mapping"
    type: "mapping"
    required: true
---

Return `(key, value)` pairs of a mapping. Works both as a filter (`obj | items`) and as a global function (`items(obj)`), making it possible to iterate over a mapping's entries in a `for` loop.

## Usage

```jinja
{% for key, value in items(my_dict) %}
  {{ key }}: {{ value }}
{% endfor %}
```
