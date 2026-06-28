---
name: "cycler"
category: "function"
signature: "cycler(*items)"
since: "2.1"
params:
  - name: "items"
    type: "varargs"
    required: true
---

Cycles through a series of values on successive calls to `next()`.

## Usage

```jinja
{% set c = cycler('odd', 'even') %}
{{ c.next() }}
```
