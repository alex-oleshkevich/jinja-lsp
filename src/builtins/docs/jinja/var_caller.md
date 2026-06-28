---
name: "caller"
category: "variable"
since: "2.0"
---

A callable available inside a macro when it is invoked via a `{% call %}` block. The body of the `{% call %}` block is rendered when `caller()` is called inside the macro.

## Usage

```jinja
{% macro render_dialog() %}{{ caller() }}{% endmacro %}
{% call render_dialog() %}Dialog body content{% endcall %}
```
