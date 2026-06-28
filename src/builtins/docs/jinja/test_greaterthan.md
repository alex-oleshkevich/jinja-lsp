---
name: "greaterthan"
category: "test"
signature: "greaterthan(other)"
since: "2.10"
---

Returns true if the value is strictly greater than the given argument. The short alias `gt` performs the same test and may be used interchangeably.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if price is greaterthan(100) %}Premium item.{% endif %}
```
