---
name: "select"
category: "filter"
signature: "select(test, *args)"
since: "2.7"
params:
  - name: "test"
    type: "string"
    default: ""
    required: true
  - name: "args"
    type: "any"
    default: ""
    required: false
---

Filter a sequence by keeping only the elements for which the given test returns true. This is the complement of `reject`. Any extra arguments after the test name are forwarded to the test function.

## Usage

```jinja
{{ [1, 2, 3, 4, 5] | select('odd') | list }}
{{ values | select('greaterthan', 0) | list }}
```
