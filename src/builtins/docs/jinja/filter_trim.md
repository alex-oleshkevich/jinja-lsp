---
name: "trim"
category: "filter"
signature: "trim(chars=None)"
since: "2.0"
params:
  - name: "chars"
    type: "string"
    default: "None"
    required: false
---

Strip leading and trailing whitespace (or a specified set of characters) from a string. When `chars` is provided, each character in that string is stripped from both ends rather than whitespace. This is equivalent to Python's `str.strip()`.

## Usage

```jinja
{{ "  hello  " | trim }}
{{ path | trim('/') }}
```
