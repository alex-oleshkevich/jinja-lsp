---
name: "dictsort"
category: "filter"
signature: "dictsort(case_sensitive=False, by='key', reverse=False)"
params:
  - name: "case_sensitive"
    type: "bool"
    default: "False"
    required: false
  - name: "by"
    type: "string"
    default: "'key'"
    required: false
  - name: "reverse"
    type: "bool"
    default: "False"
    required: false
---

Sort a dictionary and yield `(key, value)` pairs. By default it sorts by key in a case-insensitive manner. Set `by='value'` to sort by value instead, and `reverse=True` to invert the sort order.

## Usage

```jinja
{% for key, value in mydict | dictsort %}
  {{ key }}: {{ value }}
{% endfor %}
```

This filter is essential for presenting dictionary data in a predictable, alphabetical order, since Python dicts do not have a guaranteed display order in templates.
