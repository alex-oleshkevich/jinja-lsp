---
name: "gt"
category: "test"
signature: "gt(other)"
since: "2.10"
---

Alias for `greaterthan`. Returns true if the value is strictly greater than the given argument. Use `greaterthan` for clarity, or `gt` for brevity.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if count is gt(0) %}There are items.{% endif %}
```
