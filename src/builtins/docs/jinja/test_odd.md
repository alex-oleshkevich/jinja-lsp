---
name: "odd"
category: "test"
signature: "odd()"
since: "2.0"
---

Returns true if the value is an odd integer. This is the counterpart to the `even` test and is commonly used for alternating row or item styles.

## Usage

```jinja
{% if loop.index is odd %}<tr class="odd">{% endif %}
```
