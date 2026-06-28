---
name: "attr"
category: "filter"
signature: "attr(name)"
params:
  - name: "name"
    type: "string"
    required: true
---

Get an attribute of an object by name. Unlike the dot notation, this filter looks up only attributes and not items, making it useful when you want to ensure attribute access semantics.

## Usage

```jinja
{{ myobject | attr("someattr") }}
```

This is particularly handy when the attribute name is stored in a variable or contains characters that would be invalid in dot notation.
