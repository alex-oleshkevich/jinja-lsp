---
name: "iterable"
category: "test"
signature: "iterable()"
since: "2.0"
---

Returns true if the value can be iterated over, including lists, tuples, strings, dictionaries, and generators. Use this before attempting to loop over a value to avoid runtime errors.

## Usage

```jinja
{% if value is iterable %}{% for item in value %}{{ item }}{% endfor %}{% endif %}
```
