---
name: "capitalize"
category: "filter"
signature: "capitalize()"
since: "2.0"
params: []
---

Capitalize the first character of a string and convert the rest to lowercase. This is useful for normalizing user-supplied text or formatting titles.

## Usage

```jinja
{{ "hello world" | capitalize }}
```

The output will be `Hello world`. Note that unlike title-case conversion, only the very first character is uppercased; all other characters are lowercased.
