---
name: "ne"
category: "test"
signature: "ne(other)"
since: "2.10"
---

Alias for `notequal`. Returns true if the value is not equal to the given argument. Use `notequal` for clarity, or `ne` for brevity in comparisons.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if status is ne("inactive") %}Show content.{% endif %}
```
