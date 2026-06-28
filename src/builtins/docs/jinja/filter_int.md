---
name: "int"
category: "filter"
signature: "int(default=0, base=10)"
params:
  - name: "default"
    type: "int"
    default: "0"
    required: false
  - name: "base"
    type: "int"
    default: "10"
    required: false
---

Convert a value to an integer. If the conversion fails, the `default` value is returned. The `base` parameter specifies the numeric base used when parsing string representations (e.g., `16` for hexadecimal).

## Usage

```jinja
{{ "42" | int }}
{{ "0xff" | int(base=16) }}
{{ user_input | int(default=0) }}
```

Use this filter to safely coerce potentially non-numeric input from forms or query parameters into integers before using them in calculations or comparisons.
