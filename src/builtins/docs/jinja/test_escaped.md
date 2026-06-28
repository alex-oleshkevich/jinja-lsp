---
name: "escaped"
category: "test"
signature: "escaped()"
since: "2.0"
---

Returns true if the value has been marked safe for HTML output, meaning it will not be escaped when rendered. Values marked with `Markup` or the `safe` filter pass this test.

## Usage

```jinja
{% if value is escaped %}{{ value }}{% else %}{{ value | e }}{% endif %}
```
