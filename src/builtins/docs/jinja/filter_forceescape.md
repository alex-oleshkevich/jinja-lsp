---
name: "forceescape"
category: "filter"
signature: "forceescape()"
params: []
---

Unconditionally apply HTML escaping to a string and mark the result as safe. Unlike `escape`, this filter always escapes the value even if it has already been marked safe, making it useful when you must escape content that might have been pre-marked safe by another operation.

## Usage

```jinja
{{ possibly_safe_string | forceescape }}
```

Use this filter when you cannot trust that a value has not been prematurely marked safe and you need to guarantee that no raw HTML passes through to the output.
