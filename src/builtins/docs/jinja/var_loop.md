---
name: "loop"
category: "variable"
since: "2.0"
scope: "for"
attrs:
  - name: "index"
    type: "int"
  - name: "index0"
    type: "int"
  - name: "revindex"
    type: "int"
  - name: "revindex0"
    type: "int"
  - name: "first"
    type: "bool"
  - name: "last"
    type: "bool"
  - name: "length"
    type: "int"
  - name: "depth"
    type: "int"
  - name: "depth0"
    type: "int"
  - name: "previtem"
    type: "any"
  - name: "nextitem"
    type: "any"
  - name: "changed"
    type: "callable"
---

Special variable available inside `{% for %}` loops that exposes metadata about the current iteration. Attributes provide the current index, total length, whether the item is first or last, neighbouring items, and more.

## Usage

```jinja
{% for item in items %}
  {{ loop.index }}: {{ item }}
  {% if loop.last %}(last){% endif %}
{% endfor %}
```
