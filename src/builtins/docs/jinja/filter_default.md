---
name: "default"
category: "filter"
signature: "default(value='', boolean=False)"
params:
  - name: "value"
    type: "any"
    default: "''"
    required: false
  - name: "boolean"
    type: "bool"
    default: "False"
    required: false
---

Return the given default value if the variable is undefined; otherwise return the variable itself. Can also be aliased as `d`. When `boolean` is `True`, the default is also used for any falsy value (empty string, `0`, `False`, etc.), not only for undefined variables.

## Usage

```jinja
{{ my_variable | default("fallback text") }}
{{ my_variable | d("fallback", boolean=True) }}
```

This is one of the most commonly used Jinja2 filters for providing safe fallback values in templates when a context variable may not be present.
