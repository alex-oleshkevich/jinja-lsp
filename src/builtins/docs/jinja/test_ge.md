---
name: "ge"
category: "test"
signature: "ge(other)"
since: "2.10"
---

Alias for `greaterthanorequalto`. Returns true if the value is greater than or equal to the given argument. Use `greaterthanorequalto` for clarity, or `ge` for brevity.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if score is ge(60) %}Passing grade.{% endif %}
```
