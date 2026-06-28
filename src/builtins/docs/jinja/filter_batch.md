---
name: "batch"
category: "filter"
signature: "batch(linecount, fill_with=None)"
params:
  - name: "linecount"
    type: "int"
    required: true
  - name: "fill_with"
    type: "any"
    default: "None"
    required: false
---

Batch items in a list into sublists of a given size. If `fill_with` is provided, the last chunk is padded to the full size with that value.

## Usage

```jinja
{% for row in items | batch(3, '&nbsp;') %}
  <tr>
    {% for col in row %}
      <td>{{ col }}</td>
    {% endfor %}
  </tr>
{% endfor %}
```

This filter is commonly used to lay out a flat list into a table-like grid structure in templates.
