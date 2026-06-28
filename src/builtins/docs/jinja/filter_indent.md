---
name: "indent"
category: "filter"
signature: "indent(width=4, first=False, blank=False)"
params:
  - name: "width"
    type: "int"
    default: "4"
    required: false
  - name: "first"
    type: "bool"
    default: "False"
    required: false
  - name: "blank"
    type: "bool"
    default: "False"
    required: false
---

Indent each line of a string by a given number of spaces. By default the first line is not indented (`first=False`) and blank lines are not indented (`blank=False`). Set either parameter to `True` to override this behaviour.

## Usage

```jinja
{{ long_text | indent(2, first=True) }}
```

This filter is especially useful when embedding multiline content into structured formats such as YAML, Markdown, or source code where indentation is significant.
