---
name: "self"
category: "variable"
since: "2.0"
---

A reference to the current template module. Allows calling macros defined in the same template from other parts of that template, similar to how you would import and call macros from an external template.

## Usage

```jinja
{% macro my_macro() %}Hello{% endmacro %}
{{ self.my_macro() }}
```
