---
name: "float"
category: "test"
signature: "float()"
since: "2.0"
---

Returns true if the value is a floating-point number. Use this to differentiate floats from integers or other numeric types before performing type-specific operations.

## Usage

```jinja
{% if value is float %}{{ value | round(2) }}{% endif %}
```
