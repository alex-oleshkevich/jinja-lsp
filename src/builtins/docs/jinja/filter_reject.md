---
name: "reject"
category: "filter"
signature: "reject(test, *args)"
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

Filter a sequence by rejecting elements for which the given test returns true. This is the inverse of `select` — items that pass the test are excluded from the result. Any additional arguments are passed to the test function.

## Usage

```jinja
{{ [1, 2, 3, 4, 5] | reject('odd') | list }}
{{ numbers | reject('greaterthan', 10) | list }}
```
