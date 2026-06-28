---
name: "xmlattr"
category: "filter"
signature: "xmlattr(d, autospace=True)"
since: "2.0"
params:
  - name: "autospace"
    type: "bool"
    default: "True"
    required: false
---

Create an SGML/XML attribute string based on a dictionary. All values that
are not `None` or `undefined` are automatically escaped. Keys are used as
attribute names. If `autospace` is true, a leading space is prepended.

## Usage

```jinja
<input {{ {'class': 'input', 'type': 'text'} | xmlattr }}>
<ul{{ {'class': 'my-list'} | xmlattr }}>
```
