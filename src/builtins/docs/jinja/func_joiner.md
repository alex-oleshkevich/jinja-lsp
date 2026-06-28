---
name: "joiner"
category: "function"
signature: "joiner(sep=', ')"
since: "2.1"
params:
  - name: "sep"
    type: "str"
    default: "', '"
    required: false
---

A helper that joins values with a separator. Returns the separator on every call except the first, making it easy to insert delimiters between items in a loop.

## Usage

```jinja
{% set j = joiner() %}
{% for item in items %}{{ j() }}{{ item }}{% endfor %}
```
