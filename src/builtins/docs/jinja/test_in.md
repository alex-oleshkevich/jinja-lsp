---
name: "in"
category: "test"
signature: "in(seq)"
since: "2.10"
---

Returns true if the value is contained within the given sequence, string, or mapping. This is equivalent to Python's `in` operator and works with lists, tuples, strings, and dictionaries.

## Parameters

| Name | Type     | Required | Description |
|------|----------|----------|-------------|
| seq  | sequence | yes      | The sequence to search within |

## Usage

```jinja
{% if role is in(["admin", "editor"]) %}Access granted.{% endif %}
```
