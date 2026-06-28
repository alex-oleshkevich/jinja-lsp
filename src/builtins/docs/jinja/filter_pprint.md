---
name: "pprint"
category: "filter"
signature: "pprint(verbose=False)"
since: "2.0"
params:
  - name: "verbose"
    type: "boolean"
    default: "False"
    required: false
---

Pretty-print a variable using Python's `pprint` module. This filter is primarily intended for debugging — it formats complex data structures like dicts and lists in a human-readable, indented form. It is not meant for production output.

## Usage

```jinja
{{ my_variable | pprint }}
```
