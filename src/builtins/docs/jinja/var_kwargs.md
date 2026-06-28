---
name: "kwargs"
category: "variable"
since: "2.0"
---

Inside a macro, `kwargs` contains any keyword arguments passed by the caller that are not declared in the macro's signature. Useful for forwarding arbitrary HTML attributes or extra options.

## Usage

```jinja
{% macro input() %}<input {{ kwargs | xmlattr }}>{% endmacro %}
{{ input(type="text", class="form-control") }}
```
