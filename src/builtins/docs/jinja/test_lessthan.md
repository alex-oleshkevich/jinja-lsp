---
name: "lessthan"
category: "test"
signature: "lessthan(other)"
since: "2.10"
---

Returns true if the value is strictly less than the given argument. The short alias `lt` performs the same test and may be used interchangeably.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if temperature is lessthan(0) %}Freezing conditions.{% endif %}
```
