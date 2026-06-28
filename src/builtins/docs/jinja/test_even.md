---
name: "even"
category: "test"
signature: "even()"
since: "2.0"
---

Returns true if the value is an even integer. This is useful for alternating styles on list items, table rows, or any repeated structure.

## Usage

```jinja
{% if loop.index is even %}<tr class="alt">{% else %}<tr>{% endif %}
```
