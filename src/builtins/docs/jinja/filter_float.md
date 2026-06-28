---
name: "float"
category: "filter"
signature: "float(default=0.0)"
params:
  - name: "default"
    type: "float"
    default: "0.0"
    required: false
---

Convert a value to a floating-point number. If the conversion fails (e.g., the value is not numeric), the `default` value is returned instead of raising an error.

## Usage

```jinja
{{ "3.14" | float }}
{{ user_input | float(default=1.0) }}
```

Useful for safely coercing string values received from forms or external data sources into numbers before performing arithmetic in templates.
