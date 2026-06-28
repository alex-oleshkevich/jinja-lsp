---
name: "selectattr"
category: "filter"
signature: "selectattr(attribute, test, *args)"
since: "2.7"
params:
  - name: "attribute"
    type: "string"
    default: ""
    required: true
  - name: "test"
    type: "string"
    default: ""
    required: false
  - name: "args"
    type: "any"
    default: ""
    required: false
---

Filter a sequence of objects by selecting those for which the specified attribute passes the given test. When only an attribute name is given without a test, objects are kept if the attribute is truthy. This is the complement of `rejectattr`.

## Usage

```jinja
{{ users | selectattr('is_active') | list }}
{{ products | selectattr('category', 'equalto', 'electronics') | list }}
```
