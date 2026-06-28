---
name: "eq"
category: "test"
signature: "eq(other)"
since: "2.0"
---

Alias for `equalto`. Returns true if the value is equal to the given argument. Use `equalto` for clarity, or `eq` for brevity in comparisons.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if value is eq("admin") %}Welcome, admin!{% endif %}
```
