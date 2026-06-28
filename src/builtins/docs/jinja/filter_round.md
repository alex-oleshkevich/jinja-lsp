---
name: "round"
category: "filter"
signature: "round(precision=0, method='common')"
since: "2.0"
params:
  - name: "precision"
    type: "integer"
    default: "0"
    required: false
  - name: "method"
    type: "string"
    default: "'common'"
    required: false
---

Round a number to a given precision. The `method` parameter controls the rounding strategy: `'common'` rounds half up (standard rounding), `'ceil'` always rounds up, and `'floor'` always rounds down. The result is always a float.

## Usage

```jinja
{{ 3.14159 | round(2) }}
{{ 2.5 | round(method='ceil') }}
```
