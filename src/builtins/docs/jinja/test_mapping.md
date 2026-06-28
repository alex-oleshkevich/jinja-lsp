---
name: "mapping"
category: "test"
signature: "mapping()"
since: "2.10"
---

Returns true if the value is a mapping type such as a dictionary. Use this to confirm a value supports key-based access before treating it as a dict.

## Usage

```jinja
{% if value is mapping %}{{ value.key }}{% endif %}
```
