---
name: "sameas"
category: "test"
signature: "sameas(other)"
since: "2.0"
---

Returns true if the value is the exact same object as the given argument, using Python's identity comparison (`is`). This is stricter than equality: two objects may be equal in value but not the same object.

## Parameters

| Name  | Type | Required | Description |
|-------|------|----------|-------------|
| other | any  | yes      | The object to compare identity against |

## Usage

```jinja
{% if value is sameas none %}Value is None.{% endif %}
```
