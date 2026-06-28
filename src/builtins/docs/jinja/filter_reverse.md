---
name: "reverse"
category: "filter"
signature: "reverse()"
since: "2.0"
params: []
---

Reverse a string or sequence. When applied to a string, the characters are returned in reverse order. When applied to a list or other iterable, the items are yielded in reverse order. The result is an iterator, so pipe through `list` if you need a list.

## Usage

```jinja
{{ "hello" | reverse }}
{{ [1, 2, 3, 4] | reverse | list }}
```
