---
name: "join"
category: "filter"
signature: "join(d='', attribute=None)"
params:
  - name: "d"
    type: "string"
    default: "''"
    required: false
  - name: "attribute"
    type: "string"
    default: "None"
    required: false
---

Join the elements of a sequence into a string, separated by the delimiter `d`. If `attribute` is provided, that attribute is extracted from each object in the sequence before joining — equivalent to chaining `map` and `join`.

## Usage

```jinja
{{ items | join(", ") }}
{{ users | join(", ", attribute="name") }}
```

This filter is a cleaner alternative to constructing comma-separated lists with loop variables and is frequently used to display tags, categories, or other collections as inline text.
