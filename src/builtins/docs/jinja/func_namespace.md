---
name: "namespace"
category: "function"
signature: "namespace(**kwargs)"
since: "2.10"
params:
  - name: "kwargs"
    type: "kwargs"
    required: false
---

Create a namespace object that allows variable assignment inside loops and other scoped blocks. Unlike regular template variables, attributes on a namespace object can be updated from inner scopes.

## Usage

```jinja
{% set ns = namespace(found=false) %}
{% for item in items %}
  {% if item.check %}{% set ns.found = true %}{% endif %}
{% endfor %}
{{ ns.found }}
```
