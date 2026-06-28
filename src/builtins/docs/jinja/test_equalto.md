---
name: "equalto"
category: "test"
signature: "equalto(other)"
since: "2.0"
---

Returns true if the value is equal to the given argument using Python's equality operator. This is the canonical equality test; `eq` is a short alias for the same test.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The value to compare against |

## Usage

```jinja
{% if status is equalto("active") %}User is active.{% endif %}
```
