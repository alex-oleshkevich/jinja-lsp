---
name: "le"
category: "test"
signature: "le(other)"
since: "2.10"
---

Alias for `lessthanorequalto`. Returns true if the value is less than or equal to the given argument. Use `lessthanorequalto` for clarity, or `le` for brevity.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if age is le(17) %}Minor.{% endif %}
```
