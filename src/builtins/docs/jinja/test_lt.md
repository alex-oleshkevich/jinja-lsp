---
name: "lt"
category: "test"
signature: "lt(other)"
since: "2.10"
---

Alias for `lessthan`. Returns true if the value is strictly less than the given argument. Use `lessthan` for clarity, or `lt` for brevity.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if stock is lt(10) %}Low stock warning.{% endif %}
```
