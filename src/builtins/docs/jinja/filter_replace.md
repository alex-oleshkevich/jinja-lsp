---
name: "replace"
category: "filter"
signature: "replace(old, new, count=None)"
since: "2.0"
params:
  - name: "old"
    type: "string"
    default: ""
    required: true
  - name: "new"
    type: "string"
    default: ""
    required: true
  - name: "count"
    type: "integer"
    default: "None"
    required: false
---

Replace occurrences of a substring within a string with a new value. By default all occurrences are replaced; provide `count` to limit the number of replacements. The filter works on the string representation of the value.

## Usage

```jinja
{{ "Hello World" | replace("World", "Jinja") }}
{{ text | replace(" ", "_", 3) }}
```
