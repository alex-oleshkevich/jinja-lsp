---
name: "slice"
category: "filter"
signature: "slice(slices, fill_with=None)"
since: "2.0"
params:
  - name: "slices"
    type: "integer"
    default: ""
    required: true
  - name: "fill_with"
    type: "any"
    default: "None"
    required: false
---

Slice a sequence into a given number of equal-length columns. If the sequence cannot be divided evenly, the last slice is padded with `fill_with`. This is especially useful for rendering items in a multi-column grid layout.

## Usage

```jinja
{% for column in items | slice(3, '') %}
  <ul>{% for item in column %}<li>{{ item }}</li>{% endfor %}</ul>
{% endfor %}
```
