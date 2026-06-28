---
name: "sequence"
category: "test"
signature: "sequence()"
since: "2.0"
---

Returns true if the value is a sequence, such as a string, list, or tuple. Unlike `iterable`, sequences also support indexing and have a defined length.

## Usage

```jinja
{% if value is sequence %}Length: {{ value | length }}{% endif %}
```
