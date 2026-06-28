---
name: "upper"
category: "filter"
signature: "upper()"
since: "2.0"
params: []
---

Convert a string to uppercase. All lowercase letters in the string are converted to their uppercase equivalents using Unicode-aware case mapping. This is equivalent to Python's `str.upper()`.

## Usage

```jinja
{{ "hello world" | upper }}
{{ username | upper }}
```
