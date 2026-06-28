---
name: "list"
category: "filter"
signature: "list(value)"
since: "2.0"
---

Convert the value into a list. If it was a string the returned list will be
a list of characters.

## Usage

```jinja
{% for char in "hello" | list %}{{ char }}{% endfor %}
{% set items = generator | list %}
```
