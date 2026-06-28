---
name: "range"
category: "function"
signature: "range(start=0, stop, step=1)"
since: "2.0"
params:
  - name: "stop"
    type: "int"
    required: true
  - name: "start"
    type: "int"
    default: "0"
    required: false
  - name: "step"
    type: "int"
    default: "1"
    required: false
---

Works the same as Python's built-in `range()` function. Returns a list of integers from `start` up to (but not including) `stop`, incrementing by `step`.

## Usage

```jinja
{% for i in range(10) %}{{ i }}{% endfor %}
```
