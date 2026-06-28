---
name: "divisibleby"
category: "test"
signature: "divisibleby(num)"
since: "2.0"
---

Returns true if the value is divisible by the given number with no remainder. This is commonly used to apply alternating styles to list items, such as highlighting every third row.

## Parameters

| Name | Type | Required | Description |
|------|------|----------|-------------|
| num  | int  | yes      | The divisor to test against |

## Usage

```jinja
{% if loop.index is divisibleby(3) %}Every third item.{% endif %}
```
