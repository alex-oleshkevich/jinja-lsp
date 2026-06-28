---
name: "varargs"
category: "variable"
since: "2.0"
---

Inside a macro, `varargs` contains any extra positional arguments passed by the caller that are not declared in the macro's signature.

## Usage

```jinja
{% macro tag(name) %}
  <{{ name }}>{% for arg in varargs %}{{ arg }}{% endfor %}</{{ name }}>
{% endmacro %}
{{ tag('div', 'hello', 'world') }}
```
