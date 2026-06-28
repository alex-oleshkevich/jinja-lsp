---
name: "length"
category: "filter"
signature: "length()"
params: []
---

Return the number of items in a sequence or mapping. Also available as the alias `count`. Works on strings (returning character count), lists, tuples, dicts, and any other object that supports `len()`.

## Usage

```jinja
{{ my_list | length }}
{{ my_list | count }}
```

Commonly used in conditionals to check whether a collection is non-empty, or to display counts to users such as "5 results found".
