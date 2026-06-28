---
name: "defined"
category: "test"
signature: "defined()"
since: "2.0"
---

Returns true if the variable is defined in the current context. Use this test to safely check whether a variable exists before accessing it, avoiding `UndefinedError`.

## Usage

```jinja
{% if value is defined %}{{ value }}{% endif %}
```
