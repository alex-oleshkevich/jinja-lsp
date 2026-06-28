---
name: "map"
category: "filter"
signature: "map(attribute=None, default=None)"
since: "2.7"
params:
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
  - name: "default"
    type: "any"
    default: "None"
    required: false
---

Apply a filter or look up an attribute on each element of a sequence. This is useful when you want to transform every item in a list without using a full loop. When called with `attribute`, it extracts that attribute from each object; when called with a filter name, it applies that filter to each element.

## Usage

```jinja
{{ [1, 2, 3] | map('pow', 2) | list }}
{{ users | map(attribute='name') | list }}
```
